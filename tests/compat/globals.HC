// EXPECT: counter: 0
// EXPECT: counter: 1
// EXPECT: counter: 3
// EXPECT: msg: hello
I64 counter = 0;

U0 Increment(I64 by)
{
  counter += by;
}

U0 Main()
{
  Print("counter: %d\n", counter);
  Increment(1);
  Print("counter: %d\n", counter);
  Increment(2);
  Print("counter: %d\n", counter);

  Print("msg: %s\n", "hello");
}
