// EXPECT: x=10 y=20
// EXPECT: distance_sq=500
// EXPECT: sizeof(Point)=16
// EXPECT: typedef ok: 99
// EXPECT: ptr->x=7
// EXPECT: nested: inner.x=3 inner.y=4

class Point {
  I64 x;
  I64 y;
};

class Line {
  Point a;
  Point b;
};

typedef I64 Scalar;

I64 DistanceSq(Point* p)
{
  return p->x * p->x + p->y * p->y;
}

U0 Main()
{
  // Basic struct field write/read via dot syntax
  Point p;
  p.x = 10;
  p.y = 20;
  Print("x=%d y=%d\n", p.x, p.y);

  // Pass pointer to function, use ->
  I64 dsq = DistanceSq(&p);
  Print("distance_sq=%d\n", dsq);

  // sizeof a struct type
  Print("sizeof(Point)=%d\n", sizeof(Point));

  // typedef alias used as a variable type
  Scalar s = 99;
  Print("typedef ok: %d\n", s);

  // Pointer-to-struct field access via ->
  Point* pp = MAlloc(sizeof(Point));
  pp->x = 7;
  pp->y = 8;
  Print("ptr->x=%d\n", pp->x);
  Free(pp);

  // Nested struct: access inner struct fields
  Line ln;
  ln.a.x = 3;
  ln.a.y = 4;
  Print("nested: inner.x=%d inner.y=%d\n", ln.a.x, ln.a.y);
}
