# safe_app_node


## Arch Linux and possibly other OSes (2019-01-13)

Compiling this library might yield an error about telling to 'recompile with -fPIC'. This happens on Arch Linux, but might happen on other systems too. The solution is to clean and run again with specifying [an environment variable](https://github.com/maidsafe/rust_sodium/tree/ed5919ff9f713461026f84401e7bc596bdb02a08#note-for-building-on-linux):

```
> neon clean                            # This also cleans Cargo
> RUST_SODIUM_DISABLE_PIE=1 neon build  # Build libsodium differently
```


## Example

```
const safe = require('safe_app_node');

safe.app_is_mock(); // returns boolean
```
