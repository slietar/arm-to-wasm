- Use UDF to detect the end of a function
- Conventions
  - BL/RET pairing — nearly always used together. The compiler manages the call stack predictably.
  - LR handling — compilers either save LR to the stack at function entry (if the function makes further calls) or leave it untouched as a simple leaf function. Direct modification of LR is essentially never emitted.
  - SP alignment — maintained at 16-byte boundaries at call sites, as required by the AArch64 ABI.
  - Register conventions — x0–x7 for args/return, x19–x28 callee-saved, x9–x15 caller-saved scratch. Compilers follow these rigorously.
- Function prologue/epilogue patterns — compilers emit predictable sequences for setting up stack frames and saving/restoring registers. These can be used to identify function boundaries.
  - At the start: `sub sp, sp, #0x60` + `stp x29, x30, [sp, #0x50]` or a single `stp x29, x30, [sp, #-48]!`
  - At the end: `ldp x29, x30, [sp], #32`
  - Leaf functions may omit stack manipulation and simply return with `ret` if they don't call other functions.
- [Procedure Call Standard](https://developer.arm.com/documentation/102374/0103/Procedure-Call-Standard)
  - Callee-saved = the callee must preserve these registers if it uses them
  ```
  x0–x7    args/return        caller-saved
  x8       indirect result    caller-saved
  x9–x15   scratch            caller-saved
  x16–x17  linker scratch     caller-saved (hands off)
  x18      platform           reserved or caller-saved
  x19–x28  preserved          callee-saved  ← must save/restore
  x29      frame pointer      callee-saved
  x30      link register      callee-saved (by convention, saved when needed)
  sp       stack pointer      always valid, 16-byte aligned at calls
  xzr      zero register      reads 0, discards writes
  NZCV     condition flags      caller-saved
  ```
- Dynamic dispatch
  ```
  ldr     x8, [x0]         ; x8 = vptr (64-bit load)
  ldr     x8, [x8, #16]    ; x8 = vptr[2] = speak() — offset 16 = 2×8 bytes
  blr     x8               ; branch-link to register
  ```
- Tail calls - single `B` instead of `BL` + `RET`
- Swap / exchange loop
  ```
  ldur  x9,  [x8, #-0x38]       // load from region A
  ldr   x10, [x27, #0x28]       // load from region B
  str   x9,  [x27, #0x28]       // store A's value → region B
  stur  x10, [x8, #-0x38]       // store B's value → region A
  ```
- Memory copy
  ```
  ldp  q0, q1, [x1]         // load 32 bytes from src
  ldp  q2, q3, [x1, #0x20]  // load next 32 bytes
  stp  q0, q1, [x0]         // store 32 bytes to dst
  stp  q2, q3, [x0, #0x20]  // store next 32 bytes
  ```

- Required analyses
  - Whether there are B to routines - that would be harder without symtab
  - Stack size
  - List and sizes of x0-x7 arguments
  - Argument values

```sh
$ cargo run disassemble (lima cat (lima which ls) | psub)
```


## References

- https://github.com/i-net-software/JWebAssembly
- https://github.com/leaningtech/cheerpj-meta
- https://github.com/leaningtech/webvm
- https://github.com/lifting-bits/remill
- https://github.com/mirkosertic/Bytecoder
- https://teavm.org/
- https://arm64.syscall.sh/
- [AArch64 Bitmask Immediates](https://kddnewton.com/2022/08/11/aarch64-bitmask-immediates.html)
- [ELF for the Arm® 64-bit Architecture (AArch64)](https://github.com/ARM-software/abi-aa/blob/daa7a94ca55973736c0e434a67a6e4bbcd35d7fa/aaelf64/aaelf64.rst)
- [Procedure Call Standard for the Arm® 64-bit Architecture (AArch64)](https://github.com/ARM-software/abi-aa/blob/daa7a94ca55973736c0e434a67a6e4bbcd35d7fa/aapcs64/aapcs64.rst#data-types-and-alignment)
- [System V ABI for the Arm® 64-bit Architecture (AArch64)](https://github.com/ARM-software/abi-aa/blob/daa7a94ca55973736c0e434a67a6e4bbcd35d7fa/sysvabi64/sysvabi64.rst#get-the-value-of-a-symbol-defined-in-the-same-elf-file)
- https://airbus-seclab.github.io/qemu_blog/tcg_p1.html
- https://github.com/sunfishcode/wasm-reference-manual/blob/master/WebAssembly.md
- https://armasm.com/
- https://github.com/WebAssembly/binaryen/wiki/Compiling-to-WebAssembly-with-Binaryen#cfg-api
- https://en.wikipedia.org/wiki/Control-flow_graph#Reducibility
