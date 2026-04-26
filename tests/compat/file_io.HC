// EXPECT: wrote 12 bytes
// EXPECT: read back: hello world!
// EXPECT: seeked ok
U0 Main()
{
  // Write
  I64 fd = FileOpen("/tmp/_holyc_test.txt", "w");
  I64 n = FileWrite(fd, "hello world!", 12);
  Print("wrote %d bytes\n", n);
  FileClose(fd);

  // Read back (interpreter: FileRead returns a string value directly)
  fd = FileOpen("/tmp/_holyc_test.txt", "r");
  Print("read back: %s\n", FileRead(fd, 0, 64));
  FileClose(fd);

  // Seek
  fd = FileOpen("/tmp/_holyc_test.txt", "r");
  I64 pos = FileSeek(fd, 6, 0);
  if (pos == 6)
    Print("seeked ok\n");
  FileClose(fd);
}
