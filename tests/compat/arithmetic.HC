// EXPECT: 3 + 4 = 7
// EXPECT: 10! = 3628800
// EXPECT: v = 50
I64 Add(I64 a, I64 b)
{
  return a + b;
}

I64 Factorial(I64 n)
{
  if (n <= 1)
    return 1;
  return n * Factorial(n - 1);
}

U0 Main()
{
  I64 x = Add(3, 4);
  Print("3 + 4 = %d\n", x);

  I64 f = Factorial(10);
  Print("10! = %d\n", f);

  // Compound assignment: ((100+50-25)*2)/5 = 50
  I64 v = 100;
  v += 50;
  v -= 25;
  v *= 2;
  v /= 5;
  Print("v = %d\n", v);
}
