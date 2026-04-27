// EXPECT: wrote 12 bytes
// EXPECT: read back: hello world!
// EXPECT: seeked ok
U0 Main()
{
  // Use a relative path so this works on all platforms (no /tmp on Windows).
  I64 fd = FileOpen("_holyc_test_tmp.txt", "w");
  I64 n = FileWrite(fd, "hello world!", 12);
  Print("wrote %d bytes\n", n);
  FileClose(fd);

  fd = FileOpen("_holyc_test_tmp.txt", "r");
  Print("read back: %s\n", FileRead(fd, 0, 64));
  FileClose(fd);

  fd = FileOpen("_holyc_test_tmp.txt", "r");
  I64 pos = FileSeek(fd, 6, 0);
  if (pos == 6)
    Print("seeked ok\n");
  FileClose(fd);
}
