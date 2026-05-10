// EXPECT: and: 8
// EXPECT: or: 14
// EXPECT: xor: 6
// EXPECT: not: -13
// EXPECT: shl: 48
// EXPECT: shr: 3
// EXPECT: compound: 15
U0 Main()
{
  I64 a = 0b1100;
  I64 b = 0b1010;
  Print("and: %d\n",  a & b);
  Print("or: %d\n",   a | b);
  Print("xor: %d\n",  a ^ b);
  Print("not: %d\n",  ~a);
  Print("shl: %d\n",  3 << 4);
  Print("shr: %d\n",  24 >> 3);

  I64 x = 5;
  x |= 0b1010;
  Print("compound: %d\n", x);
}
