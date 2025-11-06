#include "espresso.h"

/*
 * Thread-local variable accessors
 * 
 * These functions provide safe access to thread-local global variables
 * from Rust FFI. Each function returns a pointer to the thread-local
 * variable for the current thread.
 */

/* Core data structures */
struct cube_struct* get_cube(void) {
    return &cube;
}

struct cdata_struct* get_cdata(void) {
    return &cdata;
}

/* Configuration flags accessors */
unsigned int* get_debug_ptr(void) {
    return &debug;
}

void set_debug(unsigned int value) {
    debug = value;
}

bool* get_verbose_debug_ptr(void) {
    return &verbose_debug;
}

void set_verbose_debug(bool value) {
    verbose_debug = value;
}

bool* get_trace_ptr(void) {
    return &trace;
}

void set_trace(bool value) {
    trace = value;
}

bool* get_summary_ptr(void) {
    return &summary;
}

void set_summary(bool value) {
    summary = value;
}

bool* get_remove_essential_ptr(void) {
    return &remove_essential;
}

void set_remove_essential(bool value) {
    remove_essential = value;
}

bool* get_force_irredundant_ptr(void) {
    return &force_irredundant;
}

void set_force_irredundant(bool value) {
    force_irredundant = value;
}

bool* get_unwrap_onset_ptr(void) {
    return &unwrap_onset;
}

void set_unwrap_onset(bool value) {
    unwrap_onset = value;
}

bool* get_single_expand_ptr(void) {
    return &single_expand;
}

void set_single_expand(bool value) {
    single_expand = value;
}

bool* get_use_super_gasp_ptr(void) {
    return &use_super_gasp;
}

void set_use_super_gasp(bool value) {
    use_super_gasp = value;
}

bool* get_use_random_order_ptr(void) {
    return &use_random_order;
}

void set_use_random_order(bool value) {
    use_random_order = value;
}

bool* get_skip_make_sparse_ptr(void) {
    return &skip_make_sparse;
}

void set_skip_make_sparse(bool value) {
    skip_make_sparse = value;
}

