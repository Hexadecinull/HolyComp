// EXPECT: sum(1..100) = 5050
// EXPECT: next power of 2 after 1024 = 2048
// EXPECT: countdown finished, n = 0
// EXPECT: sum of evens [0,12) = 30
U0 Main()
{
  // for loop: sum 1..100
  I64 sum = 0;
  I64 i;
  for (i = 1; i <= 100; i++)
    sum += i;
  Print("sum(1..100) = %d\n", sum);

  // while loop: first power of 2 strictly greater than 1024
  I64 p = 1;
  while (p <= 1024)
  {
    p *= 2;
  }
  Print("next power of 2 after 1024 = %d\n", p);

  // do-while countdown
  I64 n = 5;
  do {
    n--;
  } while (n > 0);
  Print("countdown finished, n = %d\n", n);

  // break / continue: sum even numbers < 12
  I64 evens = 0;
  for (i = 0; i < 20; i++)
  {
    if (i % 2 != 0) continue;
    if (i == 12)    break;
    evens += i;
  }
  Print("sum of evens [0,12) = %d\n", evens);
}
