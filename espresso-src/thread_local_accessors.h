#ifndef THREAD_LOCAL_ACCESSORS_H
#define THREAD_LOCAL_ACCESSORS_H

#include "espresso.h"

/*
 * Thread-local variable accessors
 * 
 * These functions provide safe access to thread-local global variables
 * from Rust FFI. Instead of accessing globals directly (which doesn't
 * work properly with bindgen and _Thread_local), use these functions.
 */

/* Core data structures */
struct cube_struct* get_cube(void);
struct cdata_struct* get_cdata(void);

/* Compiled architecture width — returns BPI (bits per integer) to verify host machine word width */
int get_bpi(void);

/* Configuration flags - getters return pointers for backwards compatibility */
unsigned int* get_debug_ptr(void);
void set_debug(unsigned int value);

bool* get_verbose_debug_ptr(void);
void set_verbose_debug(bool value);

bool* get_trace_ptr(void);
void set_trace(bool value);

bool* get_summary_ptr(void);
void set_summary(bool value);

bool* get_remove_essential_ptr(void);
void set_remove_essential(bool value);

bool* get_force_irredundant_ptr(void);
void set_force_irredundant(bool value);

bool* get_unwrap_onset_ptr(void);
void set_unwrap_onset(bool value);

bool* get_single_expand_ptr(void);
void set_single_expand(bool value);

bool* get_use_super_gasp_ptr(void);
void set_use_super_gasp(bool value);

bool* get_use_random_order_ptr(void);
void set_use_random_order(bool value);

bool* get_skip_make_sparse_ptr(void);
void set_skip_make_sparse(bool value);

/*
 * Recoverable-fatal guard
 *
 * The C core reports unrecoverable conditions by calling fatal() (cvrmisc.c),
 * which prints to stderr and exit()s. The Rust bindings need to survive some
 * of these (invalid input reaching the minimiser), so a thread-local recovery
 * point can be armed around a pure-C region. When armed, fatal() captures its
 * message and longjmps back to the guard instead of exiting.
 *
 * These two helpers are used by fatal() itself:
 *   - espresso_fatal_guard_armed() reports whether a recovery point is armed;
 *   - espresso_fatal_guard_trigger() copies the message into a thread-local
 *     buffer, disarms the guard, and longjmps (it does not return).
 *
 * The setjmp always lives inside the guarded_* trampolines below, so a longjmp
 * only ever unwinds C frames and never crosses a Rust frame.
 */
bool espresso_fatal_guard_armed(void);
void espresso_fatal_guard_trigger(const char* s);

/*
 * Guarded trampolines
 *
 * Each of these arms the recovery point, calls the corresponding library entry
 * point, and disarms on both normal return and catch. On a normal return the
 * result pointer is returned and *msg_out is set to NULL. On a caught fatal the
 * function returns NULL and *msg_out points to a thread-local buffer holding the
 * captured message (valid until the next fatal on this thread).
 */
pset_family guarded_espresso(pset_family F, pset_family D, pset_family R,
                             const char** msg_out);
pset_family guarded_minimize_exact(pset_family F, pset_family D, pset_family R,
                                   int exact_cover, const char** msg_out);
pset_family guarded_complement(pset* T, const char** msg_out);
pset_family guarded_primes(pset* T, const char** msg_out);

#endif /* THREAD_LOCAL_ACCESSORS_H */

