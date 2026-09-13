#include <cups/cups.h>
#include <cups/http.h>
#include <cups/ipp.h>

// CUPS 3 only; CUPS 2 has no equivalent header.
#ifdef CUPS_RS_CUPS3
#include <cups/dnssd.h>
#endif
