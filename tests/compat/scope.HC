// EXPECT: inner: 10
// EXPECT: outer: 5
// EXPECT: shadow: 99
// EXPECT: restored: 5
U0 Main()
{
  I64 x = 5;
  {
    I64 x = 10;
    Print("inner: %d\n", x);
  }
  Print("outer: %d\n", x);

  I64 y = 5;
  if (TRUE)
  {
    I64 y = 99;
    Print("shadow: %d\n", y);
  }
  Print("restored: %d\n", y);
}
