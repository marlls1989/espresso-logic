//! Cover types and traits for Boolean function minimization
//!
//! This module provides the high-level API for working with covers (sum-of-products representations
//! of Boolean functions). It includes compile-time checked builders and dynamic covers loaded from files.

use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::pla::PLASerializable;
use crate::EspressoConfig;

/// Type alias for complex cube iterator return type
pub type CubeIterator<'a> = Box<dyn Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)> + 'a>;

/// Represents the type of PLA output format (also used as cover type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PLAType {
    /// On-set only (F)
    F = 1,
    /// On-set and don't-care set (FD)
    FD = 3,
    /// On-set and off-set (FR)
    FR = 5,
    /// On-set, don't-care set, and off-set (FDR)
    FDR = 7,
}

impl PLAType {
    /// Check if this type includes F (ON-set)
    pub fn has_f(&self) -> bool {
        matches!(self, PLAType::F | PLAType::FD | PLAType::FR | PLAType::FDR)
    }

    /// Check if this type includes D (don't-care set)
    pub fn has_d(&self) -> bool {
        matches!(self, PLAType::FD | PLAType::FDR)
    }

    /// Check if this type includes R (OFF-set)
    pub fn has_r(&self) -> bool {
        matches!(self, PLAType::FR | PLAType::FDR)
    }
}

/// Type of a cube (ON-set, DC-set, or OFF-set)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CubeType {
    F, // ON-set cube
    D, // Don't-care set cube
    R, // OFF-set cube
}

/// A cube in a PLA cover
#[derive(Clone, Debug)]
pub struct Cube {
    pub(crate) inputs: Arc<[Option<bool>]>,
    pub(crate) outputs: Arc<[bool]>, // Simplified: true = bit set, false = bit not set
    pub(crate) cube_type: CubeType,
}

impl Cube {
    pub(crate) fn new(inputs: Vec<Option<bool>>, outputs: Vec<bool>, cube_type: CubeType) -> Self {
        Cube {
            inputs: inputs.into(),
            outputs: outputs.into(),
            cube_type,
        }
    }
}

/// Internal trait for types that can be minimized
/// Contains implementation details needed by the minimization algorithm
pub(crate) trait Minimizable: Send + Sync {
    /// Get the number of inputs (required for minimization)
    fn num_inputs(&self) -> usize;

    /// Get the number of outputs (required for minimization)
    fn num_outputs(&self) -> usize;

    /// Get the cover type (required for minimization)
    fn cover_type(&self) -> PLAType;

    /// Iterate over typed cubes (internal use only)
    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a>;

    /// Set cubes after minimization (internal use)
    fn set_cubes(&mut self, cubes: Vec<Cube>);
}

/// Public trait for all cover types (static and dynamic dimensions)
pub trait Cover: Send + Sync {
    /// Get the number of inputs
    fn num_inputs(&self) -> usize;

    /// Get the number of outputs  
    fn num_outputs(&self) -> usize;

    /// Get the number of cubes (for F/FD types, only counts F cubes; for FR/FDR, counts all)
    fn num_cubes(&self) -> usize;

    /// Get the cover type (F, FD, FR, or FDR)
    fn cover_type(&self) -> PLAType;

    /// Iterate over cubes (inputs, outputs)
    /// Returns cubes in same format as add_cube takes (owned vecs for easy use)
    fn cubes_iter<'a>(&'a self) -> CubeIterator<'a>;

    /// Minimize this cover in-place using default configuration
    fn minimize(&mut self) -> io::Result<()>;

    /// Minimize this cover in-place with custom configuration
    fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()>;

    /// Write this cover to PLA format using a writer
    ///
    /// This is the core serialization method that writes directly to any `Write` implementation.
    /// Both `to_pla_string` and `to_pla_file` delegate to this method for efficient serialization.
    ///
    /// # Example
    ///
    /// ```
    /// use espresso_logic::{Cover, PLACover, PLAType};
    /// use std::io::Write;
    ///
    /// let cover = PLACover::new(2, 1);
    /// let mut buffer = Vec::new();
    /// cover.write_pla(&mut buffer, PLAType::F).unwrap();
    /// let output = String::from_utf8(buffer).unwrap();
    /// println!("{}", output);
    /// ```
    fn write_pla<W: Write>(&self, writer: &mut W, pla_type: PLAType) -> io::Result<()>;

    /// Write this cover to PLA format string
    fn to_pla_string(&self, pla_type: PLAType) -> io::Result<String>;

    /// Write this cover to a PLA file
    fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()>;
}

/// Blanket implementation: Cover for all Minimizable types
impl<T: Minimizable + PLASerializable> Cover for T {
    fn num_inputs(&self) -> usize {
        Minimizable::num_inputs(self)
    }

    fn num_outputs(&self) -> usize {
        Minimizable::num_outputs(self)
    }

    fn num_cubes(&self) -> usize {
        let cover_type = self.cover_type();
        if cover_type.has_r() {
            self.internal_cubes_iter().count()
        } else {
            // F/FD: only count F cubes
            self.internal_cubes_iter()
                .filter(|cube| cube.cube_type == CubeType::F)
                .count()
        }
    }

    fn cover_type(&self) -> PLAType {
        Minimizable::cover_type(self)
    }

    fn cubes_iter<'a>(&'a self) -> CubeIterator<'a> {
        // Convert internal Cube structs to public API format
        // Only return F cubes for F-type covers, all cubes for FD/FR/FDR
        let cover_type = self.cover_type();
        Box::new(
            self.internal_cubes_iter()
                .filter(move |cube| {
                    // For F-type, only return F cubes; for FD/FR/FDR, return all
                    cover_type != PLAType::F || cube.cube_type == CubeType::F
                })
                .map(|cube| {
                    // Convert bool outputs back to Option<bool> for public API
                    let inputs = cube.inputs.to_vec();
                    let outputs: Vec<Option<bool>> =
                        cube.outputs.iter().map(|&b| Some(b)).collect();
                    (inputs, outputs)
                }),
        )
    }

    fn minimize(&mut self) -> io::Result<()> {
        let config = EspressoConfig::default();
        self.minimize_with_config(&config)
    }

    fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()> {
        use crate::r#unsafe::{UnsafeCover, UnsafeEspresso};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.internal_cubes_iter() {
            let input_vec: Vec<u8> = cube
                .inputs
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Direct C calls - thread-safe via thread-local storage
        let mut esp =
            UnsafeEspresso::new_with_config(self.num_inputs(), self.num_outputs(), config);

        // Build covers from cube data
        let f_cover = UnsafeCover::build_from_cubes(f_cubes, self.num_inputs(), self.num_outputs());
        let d_cover = if !d_cubes.is_empty() {
            Some(UnsafeCover::build_from_cubes(
                d_cubes,
                self.num_inputs(),
                self.num_outputs(),
            ))
        } else {
            None
        };
        let r_cover = if !r_cubes.is_empty() {
            Some(UnsafeCover::build_from_cubes(
                r_cubes,
                self.num_inputs(),
                self.num_outputs(),
            ))
        } else {
            None
        };

        // Minimize
        let (f_result, d_result, r_result) = esp.minimize(f_cover, d_cover, r_cover);

        // Direct conversion to typed Cubes - no serialization needed!
        let mut all_cubes = Vec::new();
        all_cubes.extend(f_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::F));
        all_cubes.extend(d_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::D));
        all_cubes.extend(r_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::R));

        // Update cubes with type information preserved
        self.set_cubes(all_cubes);
        Ok(())
    }

    fn write_pla<W: Write>(&self, writer: &mut W, pla_type: PLAType) -> io::Result<()> {
        PLASerializable::write_pla(self, writer, pla_type)
    }

    fn to_pla_string(&self, pla_type: PLAType) -> io::Result<String> {
        PLASerializable::to_pla_string(self, pla_type)
    }

    fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()> {
        PLASerializable::to_pla_file(self, path, pla_type)
    }
}

/// Marker trait for cover type specification
pub trait CoverTypeMarker: Send + Sync + Clone {
    const PLA_TYPE: PLAType;
}

/// Marker type for F (ON-set only) covers
#[derive(Clone, Copy, Debug)]
pub struct FType;
impl CoverTypeMarker for FType {
    const PLA_TYPE: PLAType = PLAType::F;
}

/// Marker type for FD (ON-set + Don't-care) covers (default)
#[derive(Clone, Copy, Debug)]
pub struct FDType;
impl CoverTypeMarker for FDType {
    const PLA_TYPE: PLAType = PLAType::FD;
}

/// Marker type for FR (ON-set + OFF-set) covers
#[derive(Clone, Copy, Debug)]
pub struct FRType;
impl CoverTypeMarker for FRType {
    const PLA_TYPE: PLAType = PLAType::FR;
}

/// Marker type for FDR (ON-set + Don't-care + OFF-set) covers
#[derive(Clone, Copy, Debug)]
pub struct FDRType;
impl CoverTypeMarker for FDRType {
    const PLA_TYPE: PLAType = PLAType::FDR;
}

/// A cover builder with compile-time dimension checking
///
/// Uses const generics for ergonomic hand-construction of covers.
/// For loading from PLA files with dynamic dimensions, use `PLACover::from_pla_*`.
///
/// The `T` type parameter specifies the cover type and defaults to `FDType` (ON-set + Don't-care).
///
/// # Examples
///
/// ```
/// use espresso_logic::{CoverBuilder, Cover, FType, FDType};
///
/// # fn main() -> std::io::Result<()> {
/// // Create a cover for a 2-input, 1-output function (FD type by default)
/// let mut cover = CoverBuilder::<2, 1>::new();
///
/// // Build the function (XOR)
/// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);  // 01 -> 1
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);  // 10 -> 1
///
/// // Or create an F-type cover explicitly
/// let mut f_cover = CoverBuilder::<2, 1, FType>::new();
/// f_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
///
/// // Minimize it
/// cover.minimize()?;
///
/// // Read the result
/// println!("Minimized to {} cubes", cover.num_cubes());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CoverBuilder<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker = FDType> {
    /// Cube data stored internally as typed cubes
    cubes: Vec<Cube>,
    _marker: std::marker::PhantomData<T>,
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker>
    CoverBuilder<INPUTS, OUTPUTS, T>
{
    /// Create a new empty cover
    pub fn new() -> Self {
        CoverBuilder {
            cubes: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Add a cube to the cover
    ///
    /// Outputs can use PLA-style notation:
    /// - `Some(true)` or `'1'` → bit set in F cube (ON-set)
    /// - `Some(false)` or `'0'` → bit set in R cube (OFF-set, only if cover type includes R)
    /// - `None` or `'-'` → bit set in D cube (Don't-care, only if cover type includes D)
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values: `Some(false)` = 0, `Some(true)` = 1, `None` = don't care
    /// * `outputs` - Output values following PLA conventions based on cover type
    pub fn add_cube(
        &mut self,
        inputs: &[Option<bool>; INPUTS],
        outputs: &[Option<bool>; OUTPUTS],
    ) -> &mut Self {
        // Parse outputs following Espresso C convention (cvrin.c lines 176-199)
        // Create separate F, D, R cubes from a single line based on output values
        let mut f_outputs = Vec::with_capacity(OUTPUTS);
        let mut d_outputs = Vec::with_capacity(OUTPUTS);
        let mut r_outputs = Vec::with_capacity(OUTPUTS);
        let mut has_f = false;
        let mut has_d = false;
        let mut has_r = false;

        let pla_type = T::PLA_TYPE;
        for &out in outputs.iter() {
            match out {
                Some(true) if pla_type.has_f() => {
                    // '1' → bit set in F cube
                    f_outputs.push(true);
                    d_outputs.push(false);
                    r_outputs.push(false);
                    has_f = true;
                }
                Some(false) if pla_type.has_r() => {
                    // '0' → bit set in R cube
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(true);
                    has_r = true;
                }
                None if pla_type.has_d() => {
                    // None/'-' → bit set in D cube
                    f_outputs.push(false);
                    d_outputs.push(true);
                    r_outputs.push(false);
                    has_d = true;
                }
                _ => {
                    // Type not supported or unset bit
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
            }
        }

        // Add cubes only if they have meaningful outputs
        let inputs_vec = inputs.to_vec();
        if has_f {
            self.cubes
                .push(Cube::new(inputs_vec.clone(), f_outputs, CubeType::F));
        }
        if has_d {
            self.cubes
                .push(Cube::new(inputs_vec.clone(), d_outputs, CubeType::D));
        }
        if has_r {
            self.cubes
                .push(Cube::new(inputs_vec, r_outputs, CubeType::R));
        }

        self
    }
}

// Implement Minimizable for CoverBuilder (Cover trait is auto-implemented via blanket impl)
impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> Minimizable
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn num_inputs(&self) -> usize {
        INPUTS
    }

    fn num_outputs(&self) -> usize {
        OUTPUTS
    }

    fn cover_type(&self) -> PLAType {
        T::PLA_TYPE
    }

    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        Box::new(self.cubes.iter())
    }

    fn set_cubes(&mut self, cubes: Vec<Cube>) {
        // Filter cubes based on the cover type
        let pla_type = T::PLA_TYPE;
        self.cubes = cubes
            .into_iter()
            .filter(|cube| match cube.cube_type {
                CubeType::F => pla_type.has_f(),
                CubeType::D => pla_type.has_d(),
                CubeType::R => pla_type.has_r(),
            })
            .collect();
    }
}

// Implement PLASerializable for CoverBuilder
impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> PLASerializable
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> Default
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> fmt::Debug
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoverBuilder")
            .field("inputs", &INPUTS)
            .field("outputs", &OUTPUTS)
            .field("cover_type", &T::PLA_TYPE)
            .field("num_cubes", &self.num_cubes())
            .finish()
    }
}
