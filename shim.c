#define R_INTERFACE_PTRS
#include <Rembedded.h>
#include <Rinterface.h>
#include <Rinternals.h>
#include <R_ext/Parse.h>
#include <R_ext/eventloop.h>

/* VECTOR_ELT is an inline/macro in R's headers, so bindgen does not
 * emit it as a function.  Expose it through a small shim so the Rust
 * side can index into an EXPRSXP returned by R_ParseVector. */
SEXP orchard_VECTOR_ELT(SEXP x, R_xlen_t i) {
    return VECTOR_ELT(x, i);
}
