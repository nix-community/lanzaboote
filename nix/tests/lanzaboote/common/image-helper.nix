{ machine, ... }:
''
  import os
  import subprocess
  import tempfile

  tmp_disk_image = tempfile.NamedTemporaryFile()

  subprocess.run([
    "${machine.virtualisation.qemu.package}/bin/qemu-img",
    "create",
    "-f",
    "qcow2",
    "-b",
    "${machine.system.build.image}/${machine.image.fileName}",
    "-F",
    "raw",
    tmp_disk_image.name,
  ])

  # Set NIX_DISK_IMAGE so that the qemu script finds the right disk image.
  os.environ['NIX_DISK_IMAGE'] = tmp_disk_image.name

  # Enroll keys via systemd-boot by rebooting
  machine.start(allow_reboot=True)
  machine.connected = False
''
