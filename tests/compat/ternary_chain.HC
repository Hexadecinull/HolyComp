// EXPECT: grade: A
// EXPECT: grade: B
// EXPECT: grade: C
// EXPECT: grade: F
U8* Grade(I64 score)
{
  return score >= 90 ? "A" :
         score >= 80 ? "B" :
         score >= 70 ? "C" : "F";
}

U0 Main()
{
  Print("grade: %s\n", Grade(95));
  Print("grade: %s\n", Grade(85));
  Print("grade: %s\n", Grade(72));
  Print("grade: %s\n", Grade(55));
}
