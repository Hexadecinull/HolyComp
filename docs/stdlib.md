# HolyComp Standard Library

All builtins are callable from HolyC source without any `#include`.

---

## Output

| Function | Signature | Description |
|---|---|---|
| `Print` | `U0 Print(U8* fmt, ...)` | Printf-style formatted output. Supports `%d %i %u %x %X %f %g %c %s %%`. |
| `SPrint` | `U0 SPrint(U8* buf, U8* fmt, ...)` | Format into a string (interpreter: returns `Value::Str`). |

---

## Math

| Function | Signature | Description |
|---|---|---|
| `Abs` | `I64 Abs(I64 x)` | Absolute value (integer or float). |
| `Sin` | `F64 Sin(F64 x)` | Sine (radians). |
| `Cos` | `F64 Cos(F64 x)` | Cosine (radians). |
| `Sqrt` | `F64 Sqrt(F64 x)` | Square root. |
| `Pow` | `F64 Pow(F64 base, F64 exp)` | Power. |

---

## Memory

| Function | Signature | Description |
|---|---|---|
| `MAlloc` | `U8* MAlloc(I64 size)` | Allocate `size` bytes; returns pointer or NULL on OOM. |
| `Free` | `U0 Free(U8* ptr)` | Free a previously allocated block. |
| `MemSet` | `U0 MemSet(U8* ptr, I64 val, I64 len)` | Fill `len` bytes at `ptr` with `val`. |
| `MemCpy` | `U0 MemCpy(U8* dst, U8* src, I64 len)` | Copy `len` bytes from `src` to `dst`. |
| `MemCmp` | `I64 MemCmp(U8* a, U8* b, I64 len)` | Compare `len` bytes; returns `<0 / 0 / >0`. |

---

## Strings

| Function | Signature | Description |
|---|---|---|
| `StrLen` | `I64 StrLen(U8* s)` | Length of null-terminated string. |
| `StrCmp` | `I64 StrCmp(U8* a, U8* b)` | Compare strings; returns `<0 / 0 / >0`. |
| `StrCpy` | `U8* StrCpy(U8* dst, U8* src)` | Copy `src` into `dst`; returns `dst`. |
| `StrCat` | `U8* StrCat(U8* a, U8* b)` | Concatenate strings; returns result. |
| `StrStr` | `I64 StrStr(U8* hay, U8* needle)` | First occurrence index or `-1`. |
| `StrToI64` | `I64 StrToI64(U8* s)` | Parse decimal integer string. |

---

## File I/O

| Function | Signature | Description |
|---|---|---|
| `FileOpen` | `I64 FileOpen(U8* path, U8* flags)` | Open file; flags: `"r"` `"w"` `"a"` `"r+"`. Returns fd or `-1`. |
| `FileClose` | `I64 FileClose(I64 fd)` | Close file descriptor. Returns `0` or `-1`. |
| `FileRead` | `I64 FileRead(I64 fd, U8* buf, I64 len)` | Read up to `len` bytes. Returns bytes read or `-1`. |
| `FileWrite` | `I64 FileWrite(I64 fd, U8* data, I64 len)` | Write `len` bytes. Returns bytes written or `-1`. |
| `FileSeek` | `I64 FileSeek(I64 fd, I64 offset, I64 whence)` | Seek; whence: 0=start 1=current 2=end. Returns new position. |

---

## Random

| Function | Signature | Description |
|---|---|---|
| `Rand` | `I64 Rand()` | Pseudo-random integer in `[0, I64_MAX]`. |
| `RandI64` | `I64 RandI64(I64 lo, I64 hi)` | Pseudo-random integer in `[lo, hi]` inclusive. |
| `SRand` | `U0 SRand(I64 seed)` | Seed the RNG. |

---

## Time

| Function | Signature | Description |
|---|---|---|
| `Time` | `I64 Time()` | Current Unix timestamp in seconds. |

---

## Program control

| Function | Signature | Description |
|---|---|---|
| `Exit` | `U0 Exit(I64 code)` | Terminate with exit code. |
