#include <stdint.h>
typedef struct SensesEngine SensesEngine;
// All calls for an engine must be serialized. Returned strings are owned by
// Rust and must be freed with senses_string_free. No audio crosses this ABI.
SensesEngine *senses_engine_create(void);
void senses_engine_destroy(SensesEngine *engine);
char *senses_command(SensesEngine *engine, const char *json);
char *senses_snapshot(SensesEngine *engine);
char *senses_sources(void);
void senses_string_free(char *value);
