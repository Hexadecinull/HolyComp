// EXPECT: 1
// EXPECT: 2
// EXPECT: 3
// EXPECT: done: 3
U0 Main()
{
  I64 i = 1;
  do {
    Print("%d\n", i);
    i++;
  } while (i <= 3);
  Print("done: %d\n", i - 1);
}
