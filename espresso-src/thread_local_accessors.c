#include "espresso.h"
#include "thread_local_accessors.h"

#include <setjmp.h>
#include <string.h>

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

/*
 * Recoverable-fatal guard
 *
 * Per-thread recovery state shared with fatal() (cvrmisc.c). The jmp_buf target
 * is established by whichever guarded_* trampoline is currently running; fatal()
 * consults fatal_armed and, when set, copies its message into fatal_message and
 * longjmps back to that trampoline. The trampolines always disarm before using
 * the state again, so at most one recovery point is armed per thread at a time.
 */
#define FATAL_MESSAGE_MAX 256
static _Thread_local jmp_buf fatal_env;
static _Thread_local bool fatal_armed = FALSE;
static _Thread_local char fatal_message[FATAL_MESSAGE_MAX];

bool espresso_fatal_guard_armed(void) {
    return fatal_armed;
}

void espresso_fatal_guard_trigger(const char* s) {
    if (s != NULL) {
        strncpy(fatal_message, s, FATAL_MESSAGE_MAX - 1);
        fatal_message[FATAL_MESSAGE_MAX - 1] = '\0';
    } else {
        fatal_message[0] = '\0';
    }
    /* Disarm before jumping so the buffer is not clobbered by a stray fatal on
     * the unwind path, and so the guard is inert once control leaves C. */
    fatal_armed = FALSE;
    longjmp(fatal_env, 1);
}

pset_family guarded_espresso(pset_family F, pset_family D, pset_family R,
                             const char** msg_out) {
    *msg_out = NULL;
    if (setjmp(fatal_env) != 0) {
        /* fatal() jumped back here; it has already disarmed the guard. */
        *msg_out = fatal_message;
        return NULL;
    }
    fatal_armed = TRUE;
    pset_family result = espresso(F, D, R);
    fatal_armed = FALSE;
    return result;
}

pset_family guarded_minimize_exact(pset_family F, pset_family D, pset_family R,
                                   int exact_cover, const char** msg_out) {
    *msg_out = NULL;
    if (setjmp(fatal_env) != 0) {
        *msg_out = fatal_message;
        return NULL;
    }
    fatal_armed = TRUE;
    pset_family result = minimize_exact(F, D, R, exact_cover);
    fatal_armed = FALSE;
    return result;
}

pset_family guarded_complement(pset* T, const char** msg_out) {
    *msg_out = NULL;
    if (setjmp(fatal_env) != 0) {
        *msg_out = fatal_message;
        return NULL;
    }
    fatal_armed = TRUE;
    pset_family result = complement(T);
    fatal_armed = FALSE;
    return result;
}

