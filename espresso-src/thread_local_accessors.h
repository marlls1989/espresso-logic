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

#endif /* THREAD_LOCAL_ACCESSORS_H */

