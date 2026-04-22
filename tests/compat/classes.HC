// EXPECT: sizeof(I64) = 8
// EXPECT: sizeof(U8) = 1
// EXPECT: sizeof(Bool) = 1
// EXPECT: sizeof(local I64) = 8
class Vec2 {
  I64 x;
  I64 y;
};

typedef I64 Index;

U0 Main()
{
  Print("sizeof(I64) = %d\n", sizeof(I64));
  Print("sizeof(U8) = %d\n",  sizeof(U8));
  Print("sizeof(Bool) = %d\n", sizeof(Bool));

  I64 sz = 99;
  Print("sizeof(local I64) = %d\n", sizeof(sz));
}
