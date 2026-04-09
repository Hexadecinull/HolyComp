// HolyC compat test: class (struct) definition and typedef
class Vec2 {
  I64 x;
  I64 y;
};

typedef I64 Index;

U0 Main()
{
  // sizeof on a known type (returns 8 on 64-bit)
  I64 sz = sizeof(I64);
  Print("sizeof(I64) = %d\n", sz);

  // sizeof via expression
  I64 arr_sz = sizeof(sz);
  Print("sizeof(local I64) = %d\n", arr_sz);
}
