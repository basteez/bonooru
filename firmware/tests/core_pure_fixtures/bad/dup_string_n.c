/* Fixture for check-core-pure.sh: must be flagged (strndup allocates on the heap). */
char *ocb_fixture_dup(const char *s);
char *ocb_fixture_dup(const char *s)
{
    return strndup(s, 4);
}
