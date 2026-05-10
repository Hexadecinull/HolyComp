// TempleOS compatibility layer smoke test.
// EXPECT: w=640 h=480
// EXPECT: ticks>=0
// EXPECT: ptr_valid: 1
// EXPECT: ptr_null: 0
// EXPECT: doc: hello from doc
// EXPECT: gr: text at 10,20
// EXPECT: slept ok
U0 Main()
{
  // Screen dimensions match canonical TempleOS values
  I64 w = GrWidth();
  I64 h = GrHeight();
  Print("w=%d h=%d\n", w, h);

  // Ticks are non-negative
  I64 t = GetTicks();
  if (t >= 0)
    Print("ticks>=0\n");
  else
    Print("ticks<0\n");

  // ChkPtr
  I64* p = MAlloc(8);
  Print("ptr_valid: %d\n", ChkPtr(p));
  Print("ptr_null: %d\n",  ChkPtr(NULL));
  Free(p);

  // DocPrint / DocPutS
  DocPrint("doc: hello from doc\n");
  DocPutS("gr: text at 10,20\n");

  // Graphics no-ops should not crash
  GrLine(0, 0, 100, 100, 0xFFFFFF);
  GrRect(10, 10, 50, 50, 0xFF0000);
  GrFillRect(20, 20, 30, 30, 0x00FF00);
  GrCircle(50, 50, 25, 0x0000FF);
  GrCls();
  GrFlush();

  // Sleep 0 ms should complete immediately
  Sleep(0);
  Print("slept ok\n");
}
