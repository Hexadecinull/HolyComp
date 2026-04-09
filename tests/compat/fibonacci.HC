// EXPECT: fib(0) = 0
// EXPECT: fib(1) = 1
// EXPECT: fib(2) = 1
// EXPECT: fib(3) = 2
// EXPECT: fib(4) = 3
// EXPECT: fib(5) = 5
// EXPECT: fib(6) = 8
// EXPECT: fib(7) = 13
// EXPECT: fib(8) = 21
// EXPECT: fib(9) = 34
// EXPECT: fib(10) = 55
// EXPECT: FibIter(15) = 610, FibRec(15) = 610
I64 FibIter(I64 n)
{
  if (n <= 1) return n;
  I64 a = 0;
  I64 b = 1;
  I64 i;
  for (i = 2; i <= n; i++)
  {
    I64 tmp = a + b;
    a = b;
    b = tmp;
  }
  return b;
}

I64 FibRec(I64 n)
{
  if (n <= 1) return n;
  return FibRec(n - 1) + FibRec(n - 2);
}

U0 Main()
{
  I64 i;
  for (i = 0; i <= 10; i++)
  {
    Print("fib(%d) = %d\n", i, FibIter(i));
  }
  I64 iter = FibIter(15);
  I64 rec  = FibRec(15);
  Print("FibIter(15) = %d, FibRec(15) = %d\n", iter, rec);
}
