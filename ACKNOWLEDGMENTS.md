# Acknowledgments

## Original Espresso Logic Minimizer

This project builds upon the **Espresso heuristic logic minimizer**, developed at the University of California, Berkeley.

### Original Copyright Notice

```
Oct Tools Distribution 3.0

Copyright (c) 1988, 1989, Regents of the University of California.
All rights reserved.

Use and copying of this software and preparation of derivative works
based upon this software are permitted. However, any distribution of
this software or derivative works must include the above copyright
notice.

This software is made available AS IS, and neither the Electronics
Research Laboratory or the University of California make any
warranty about the software, its performance or its conformity to
any specification.
```

### Original Authors and Contributors

The Espresso logic minimizer was developed by the following researchers at UC Berkeley:

- **Robert K. Brayton** - Principal Investigator
- **Gary D. Hachtel** - Co-author
- **Curtis T. McMullen** - Co-author
- **Alberto L. Sangiovanni-Vincentelli** - Co-author

And many other contributors at the Electronics Research Laboratory, UC Berkeley.

### Key Publications

The foundational work behind Espresso is described in:

1. Brayton, R. K., Hachtel, G. D., McMullen, C. T., & Sangiovanni-Vincentelli, A. L. (1984). 
   **Logic Minimization Algorithms for VLSI Synthesis**. 
   Kluwer Academic Publishers.

2. Brayton, R. K., et al. (1984). 
   **Fast Recursive Solutions to the Covering Problem**. 
   Proc. IEEE Int'l Conf. on Computer Design (ICCD).

3. Brayton, R. K., et al. (1982). 
   **Multiple-Level Logic Optimization System**. 
   Proc. IEEE Int'l Conf. on Computer-Aided Design (ICCAD).

### UC Berkeley Electronics Research Laboratory

The original Espresso was developed at:

**Electronics Research Laboratory**  
University of California, Berkeley  
Berkeley, CA 94720  
United States

Contact (historical): octtools@eros.berkeley.edu

### Recognition

We are deeply grateful to the original authors and the University of California, Berkeley, for making this powerful tool available with a permissive license that allows derivative works. The Espresso algorithm has been instrumental in digital design and logic synthesis for decades.

The original C implementation (in the `espresso-src/` directory) is preserved without modification, except as required for building in modern environments. All original copyright notices have been maintained.

## Modernized C Version

**Copyright (c) 2016 Sébastien Cottinet**

The modernized, compilable version of the Espresso C source code was created and released under the MIT License by:

**Sébastien Cottinet**  
GitHub: [@scottinet](https://github.com/scottinet) / [@sebastien-cottinet](https://github.com/sebastien-cottinet)  
Original Repository: https://github.com/scottinet/espresso-logic-minimizer

This 2016 modernization made the original 1988 C code compatible with C99 and modern compilers, which was essential for creating these Rust bindings. Sébastien Cottinet released his modernization work under the MIT License, allowing further derivative works.

## Rust Wrapper

The Rust wrapper and safe API bindings were developed by:

**Marcos Sartori**  
Newcastle University  
Email: marcos.sartori@ncl.ac.uk  
GitHub: [@marlls1989](https://github.com/marlls1989)

### Additional Acknowledgments

- **Sébastien Cottinet** - For the modernized, compilable C version (2017)
- **classabbyamp** - For maintaining the modernized C codebase
- The Rust community for excellent FFI tools (bindgen, cc crate)
- All contributors to the espresso-logic project

## License Compliance

This derived work (Rust wrapper) is distributed under the MIT License, which is compatible with the original UC Berkeley license. Both licenses require preservation of copyright notices, which are maintained in:

1. All original source files in `espresso-src/`
2. This ACKNOWLEDGMENTS file
3. The LICENSE file

## Citation

If you use this software in academic work, please cite both the original Espresso work and this implementation:

### Citing the Original Espresso

```bibtex
@book{brayton1984logic,
  title={Logic Minimization Algorithms for VLSI Synthesis},
  author={Brayton, Robert K. and Hachtel, Gary D. and McMullen, Curtis T. and Sangiovanni-Vincentelli, Alberto L.},
  year={1984},
  publisher={Kluwer Academic Publishers},
  address={Boston, MA}
}
```

### Citing This Implementation

```bibtex
@software{espresso_rust_2024,
  author={Sartori, Marcos},
  title={Espresso Logic Minimizer: Rust Bindings},
  year={2024},
  url={https://github.com/marlls1989/espresso-logic},
  note={Rust wrapper for the UC Berkeley Espresso logic minimizer}
}
```

## Related Projects

- [Original Espresso Distribution](https://embedded.eecs.berkeley.edu/pubs/downloads/espresso/index.htm)
- [ABC - A System for Sequential Synthesis and Verification](https://people.eecs.berkeley.edu/~alanmi/abc/)
- [Berkeley Logic Synthesis and Verification Group](https://people.eecs.berkeley.edu/~alanmi/)

---

*This acknowledgment file is maintained to ensure compliance with the original UC Berkeley copyright requirements and to give proper credit to all contributors to this project.*

