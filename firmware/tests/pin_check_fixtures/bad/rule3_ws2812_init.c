/* Fixture (rule 3): literal pin in ws2812_program_init. */
void f(void) { ws2812_program_init(pio, sm, offset, 16, 800000, false); }
