/* Fixture for check-core-pure.sh: must be flagged (strdup allocates on the heap). */
char *ocb_fixture_dup(const char *s);
char *ocb_fixture_dup(const char *s)
{
    return strdup(s);
}
