''
  SD_LOADER_GUID = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"
  def read_raw_variable(var: str) -> bytes:
      attr_var = machine.succeed(f"cat /sys/firmware/efi/efivars/{var}-{SD_LOADER_GUID}").encode('raw_unicode_escape')
      _ = attr_var[:4] # First 4 bytes are attributes according to https://www.kernel.org/doc/html/latest/filesystems/efivarfs.html
      value = attr_var[4:]
      return value
  def read_string_variable(var: str, encoding='utf-16-le') -> str:
      return read_raw_variable(var).decode(encoding).rstrip('\x00')
  # By default, it will read a 4 byte value, read `struct` docs to change the format.
  def assert_variable_string(var: str, expected: str, encoding='utf-16-le'):
      with subtest(f"Is `{var}` correctly set"):
          value = read_string_variable(var, encoding)
          assert value == expected, f"Unexpected variable value in `{var}`, expected: `{expected.encode(encoding)!r}`, actual: `{value.encode(encoding)!r}`"
  def assert_variable_string_contains(var: str, expected_substring: str):
      with subtest(f"Do `{var}` contain expected substrings"):
          value = read_string_variable(var).strip()
          assert expected_substring in value, f"Did not find expected substring in `{var}`, expected substring: `{expected_substring}`, actual value: `{value}`"
''
