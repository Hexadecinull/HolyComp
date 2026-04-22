// EXPECT: hello world
// EXPECT: length: 11
// EXPECT: compare equal: 1
// EXPECT: compare diff: -1
// EXPECT: concat: foobar
// EXPECT: search hit: 3
// EXPECT: search miss: -1
U0 Main()
{
  I64 n = 42;
  Print("hello world\n");

  I64 len = StrLen("hello world");
  Print("length: %d\n", len);

  I64 eq = StrCmp("abc", "abc");
  Print("compare equal: %d\n", eq == 0 ? 1 : 0);

  I64 diff = StrCmp("abc", "abd");
  Print("compare diff: %d\n", diff < 0 ? -1 : 1);

  Print("concat: %s\n", StrCat("foo", "bar"));

  I64 pos = StrStr("foobar", "bar");
  Print("search hit: %d\n", pos);

  I64 miss = StrStr("foobar", "xyz");
  Print("search miss: %d\n", miss);
}
