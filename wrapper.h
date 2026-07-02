#define R_INTERFACE_PTRS
#include <Rembedded.h>
#include <Rinterface.h>
#include <Rinternals.h>
#include <R_ext/Parse.h>
#include <R_ext/eventloop.h>

/* Declaration only; the definition lives in shim.c so that bindgen
 * emits an extern FFI binding and the linker resolves it from the
 * compiled shim object.  VECTOR_ELT itself is a macro in R's
 * headers, so bindgen cannot pick it up directly. */
SEXP orchard_VECTOR_ELT(SEXP x, R_xlen_t i);
