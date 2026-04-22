// EXPECT: gcd(48,18) = 6
// EXPECT: pow2(10) = 1024
// EXPECT: sum(100) = 5050
I64 Gcd(I64 a, I64 b)
{
  if (b == 0) return a;
  return Gcd(b, a % b);
}

I64 Pow2(I64 n)
{
  if (n == 0) return 1;
  return 2 * Pow2(n - 1);
}

I64 Sum(I64 n)
{
  if (n <= 0) return 0;
  return n + Sum(n - 1);
}

U0 Main()
{
  Print("gcd(48,18) = %d\n", Gcd(48, 18));
  Print("pow2(10) = %d\n",   Pow2(10));
  Print("sum(100) = %d\n",   Sum(100));
}
