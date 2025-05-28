- Fix the PikeVM: The bytecode was changed such that capture group 0, and the lazy start at the beginning
  are now implemented by the engine, rather than being baked into the bytecode.
  Needs to reflect that change in the interpreter.
- Re-implement capture groups. Need to choose a good API.
