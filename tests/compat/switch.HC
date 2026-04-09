// EXPECT: Sunday
// EXPECT: Wednesday
// EXPECT: Friday
// EXPECT: Unknown
U0 DayName(I64 d)
{
  switch (d)
  {
    case 0: Print("Sunday\n");    break;
    case 1: Print("Monday\n");    break;
    case 2: Print("Tuesday\n");   break;
    case 3: Print("Wednesday\n"); break;
    case 4: Print("Thursday\n");  break;
    case 5: Print("Friday\n");    break;
    case 6: Print("Saturday\n");  break;
    default: Print("Unknown\n");  break;
  }
}

U0 Main()
{
  DayName(0);
  DayName(3);
  DayName(5);
  DayName(9);
}
