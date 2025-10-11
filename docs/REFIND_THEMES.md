# Using rEFInd Themes with Lanzaboote

Lanzaboote makes it easy to install rEFInd themes from GitHub. This guide shows you how to use the built-in theme utilities.

## Quick Start: Using Pre-configured Themes

The module includes helpers for popular rEFInd themes:

```nix
{ config, lanzaboote-utils, ... }:
{
  boot.lanzaboote = {
    enable = true;
    bootloader = "refind";
    pkiBundle = "/var/lib/sbctl";

    refind = {
      # Use a pre-configured theme (you'll need to provide the sha256)
      extraFiles = lanzaboote-utils.refindThemes.minimal;

      extraConfig = ''
        include themes/refind-minimal/theme.conf
      '';
    };
  };
}
```

### Available Pre-configured Themes

- **minimal** - rEFInd-minimal by evanpurkhiser - Clean, minimal design
- **minimalist** - minimal-refind-theme by andersfischernielsen
- **regular** - refind-theme-regular by bobafetthotmail - Colorful with good icon set

> **Note**: You'll need to provide the correct `sha256` hash for these themes. See the "Getting the SHA256" section below.

## Fetching Custom Themes from GitHub

Use the `fetchRefindTheme` function to install any rEFInd theme from GitHub:

```nix
{ config, pkgs, lanzaboote-utils, ... }:
let
  myTheme = lanzaboote-utils.fetchRefindTheme {
    owner = "username";
    repo = "my-refind-theme";
    rev = "v1.0.0";  # or "main", "master", etc.
    sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    themeName = "my-theme";  # Optional, defaults to repo name
    themeSubdir = null;  # Optional, if theme is in a subdirectory
  };
in
{
  boot.lanzaboote = {
    enable = true;
    bootloader = "refind";
    pkiBundle = "/var/lib/sbctl";

    refind = {
      extraFiles = myTheme;

      extraConfig = ''
        include themes/my-theme/theme.conf
      '';
    };
  };
}
```

## Getting the SHA256 Hash

When you first use a theme, Nix will fail with a hash mismatch and show you the correct hash. Here's how to handle it:

### Method 1: Let Nix tell you

1. Use a fake hash first:
   ```nix
   sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
   ```

2. Try to build:
   ```bash
   sudo nixos-rebuild switch
   ```

3. Nix will error with the actual hash:
   ```
   hash mismatch in fixed-output derivation
   specified: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
   got:       sha256-RealHashValueWillAppearHere=
   ```

4. Copy the real hash and update your configuration.

### Method 2: Use nix-prefetch

```bash
nix-prefetch-url --unpack https://github.com/username/repo/archive/ref.tar.gz
```

## Complete Example

Here's a complete example using the rEFInd-minimal theme:

```nix
{ config, pkgs, lib, lanzaboote-utils, ... }:
let
  # Fetch the theme from GitHub
  refindMinimal = lanzaboote-utils.fetchRefindTheme {
    owner = "evanpurkhiser";
    repo = "rEFInd-minimal";
    rev = "2.1.0";
    sha256 = "sha256-abc123...";  # Replace with actual hash
    themeName = "refind-minimal";
  };
in
{
  imports = [ /* ... */ ];

  boot.loader.systemd-boot.enable = lib.mkForce false;

  boot.lanzaboote = {
    enable = true;
    bootloader = "refind";
    pkiBundle = "/var/lib/sbctl";

    refind = {
      # Install the theme files
      extraFiles = refindMinimal;

      # Configure rEFInd to use the theme
      extraConfig = ''
        # Use the theme
        include themes/refind-minimal/theme.conf

        # Additional customizations
        resolution 1920 1080
        use_graphics_for linux
        hideui hints,arrows
      '';
    };
  };

  environment.systemPackages = with pkgs; [
    sbctl
    refind
  ];
}
```

## Multiple Themes and Custom Icons

You can combine multiple sources and add custom icons:

```nix
{ config, lanzaboote-utils, ... }:
let
  # Fetch a theme
  myTheme = lanzaboote-utils.fetchRefindTheme {
    owner = "username";
    repo = "cool-theme";
    rev = "main";
    sha256 = "sha256-...";
  };
in
{
  boot.lanzaboote.refind = {
    extraFiles = myTheme // {
      # Add custom icons on top of the theme
      "icons/os_nixos.png" = ./custom-nixos-icon.png;
      "icons/os_linux.png" = ./custom-linux-icon.png;
    };

    extraConfig = ''
      include themes/cool-theme/theme.conf

      # Override theme settings
      big_icon_size 256
      small_icon_size 96
    '';
  };
}
```

## Popular rEFInd Themes

Here are some popular rEFInd themes you can use:

### rEFInd-minimal
- **Repo**: https://github.com/evanpurkhiser/rEFInd-minimal
- **Style**: Clean, minimal black and white design
- **Best for**: Minimalist setups

### minimal-refind-theme
- **Repo**: https://github.com/andersfischernielsen/minimal-refind-theme
- **Style**: Ultra-minimal with simple icons
- **Best for**: Those who want simplicity

### refind-theme-regular
- **Repo**: https://github.com/bobafetthotmail/refind-theme-regular
- **Style**: Colorful theme with comprehensive icon set
- **Best for**: Multi-boot setups with many OSes

### rEFInd-glassy
- **Repo**: https://github.com/Pr0cella/rEFInd-glassy
- **Style**: Modern glass-like design
- **Best for**: Modern, sleek aesthetics

## Troubleshooting

### Theme doesn't appear

Make sure your `extraConfig` includes the theme:
```nix
extraConfig = ''
  include themes/your-theme-name/theme.conf
'';
```

### Icons don't show up

Some themes require specific icon names. Check the theme's documentation for required icon filenames.

### Theme files not found

Verify the `themeName` matches what's in your `include` statement, and that the theme actually has a `theme.conf` file.

## Creating Your Own Theme

You can also create custom themes locally:

```nix
{
  boot.lanzaboote.refind.extraFiles = {
    "themes/custom/theme.conf" = ./mytheme/theme.conf;
    "themes/custom/background.png" = ./mytheme/background.png;
    "themes/custom/icons/os_nixos.png" = ./mytheme/icons/os_nixos.png;
  };
}
```

See the [rEFInd theming documentation](http://www.rodsbooks.com/refind/themes.html) for details on creating themes.
