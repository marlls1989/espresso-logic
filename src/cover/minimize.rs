//! Minimizable trait implementation for Cover
//!
//! This module implements the [`Minimizable`] trait for [`Cover`], providing
//! the actual minimization logic using the Espresso algorithm.

use super::cubes::CubeType;
use super::minimizable::Minimizable;
use super::Cover;
use crate::error::MinimizationError;
use crate::EspressoConfig;

// Implement public Minimizable trait for Cover
impl Minimizable for Cover {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        use crate::espresso::{Espresso, EspressoCover};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.cubes.iter() {
            let input_vec: Vec<u8> = cube
                .inputs()
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs()
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type() {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Direct C calls - thread-safe via thread-local storage
        let esp = Espresso::new(self.num_inputs(), self.num_outputs(), config);

        // Build covers from cube data
        let f_cover = EspressoCover::from_cubes(f_cubes, self.num_inputs(), self.num_outputs())?;
        let d_cover = if !d_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                d_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };
        let r_cover = if !r_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                r_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };

        // Minimize
        let (f_result, d_result, r_result) =
            esp.minimize(&f_cover, d_cover.as_ref(), r_cover.as_ref());

        // Extract minimized cubes
        let mut minimized_cubes = Vec::new();
        minimized_cubes.extend(f_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::F,
        ));
        minimized_cubes.extend(d_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::D,
        ));
        minimized_cubes.extend(r_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::R,
        ));

        // Build new cover with minimized cubes - only clone labels (Arc, cheap)
        Ok(Cover {
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            input_labels: self.input_labels.clone(),
            output_labels: self.output_labels.clone(),
            cubes: minimized_cubes,
            cover_type: self.cover_type,
        })
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        use crate::espresso::{Espresso, EspressoCover};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.cubes.iter() {
            let input_vec: Vec<u8> = cube
                .inputs()
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs()
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type() {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Direct C calls - thread-safe via thread-local storage
        let esp = Espresso::new(self.num_inputs(), self.num_outputs(), config);

        // Build covers from cube data
        let f_cover = EspressoCover::from_cubes(f_cubes, self.num_inputs(), self.num_outputs())?;
        let d_cover = if !d_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                d_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };
        let r_cover = if !r_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                r_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };

        // Minimize using exact algorithm
        let (f_result, d_result, r_result) =
            esp.minimize_exact(&f_cover, d_cover.as_ref(), r_cover.as_ref());

        // Extract minimized cubes
        let mut minimized_cubes = Vec::new();
        minimized_cubes.extend(f_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::F,
        ));
        minimized_cubes.extend(d_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::D,
        ));
        minimized_cubes.extend(r_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::R,
        ));

        // Build new cover with minimized cubes - only clone labels (Arc, cheap)
        Ok(Cover {
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            input_labels: self.input_labels.clone(),
            output_labels: self.output_labels.clone(),
            cubes: minimized_cubes,
            cover_type: self.cover_type,
        })
    }
}

// Note: Blanket implementation of Minimizable for types convertible to/from Bdd
// is provided in the bdd module (src/expression/bdd.rs)
