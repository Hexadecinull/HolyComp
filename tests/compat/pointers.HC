// EXPECT: alloc ok
// EXPECT: after memset: 0
// EXPECT: write and read: 42
// EXPECT: free ok
U0 Main()
{
  // Allocate an I64-sized block
  I64* p = MAlloc(8);
  if (p == NULL)
  {
    Print("alloc failed\n");
    return;
  }
  Print("alloc ok\n");

  // MemSet to zero and read back
  MemSet(p, 0, 8);
  I64 v = *p;
  Print("after memset: %d\n", v);

  // Write a value and read it back via dereference
  *p = 42;
  v  = *p;
  Print("write and read: %d\n", v);

  // Free the block
  Free(p);
  Print("free ok\n");
}
