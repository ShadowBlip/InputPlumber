# Installation

=== "ArchLinux"

    If you are using Arch Linux or any of its derivatives,
    you can install InputPlumber from the official repositories:
    
    ```bash
    sudo pacman -S inputplumber
    ```
    
    Then start the service with:
    
    ```bash
    sudo systemctl enable inputplumber
    sudo systemctl start inputplumber
    ```

=== "Fedora"

    If you are using Fedora, you can install InputPlumber from COPR.

    To install, first add the COPR repository:

    ```bash
    dnf copr enable shadowblip/InputPlumber
    ```

    Then you can install the package with dnf:

    ```bash
    dnf install inputplumber
    ```

    Then start the service with:
    
    ```bash
    sudo systemctl enable inputplumber
    sudo systemctl start inputplumber
    ```

=== "Debian / Ubuntu"

    If you are using Debian or any of its derivatives, you can install
    InputPlumber using the pre-built deb package.

    To install the package, first visit the [release page](https://github.com/ShadowBlip/InputPlumber/releases) and download
    the `.deb` package for your architecture.

    Then use `dpkg` to install the package:

    ```bash
    sudo dpkg -i inputplumber_*.deb
    ```

    Then start the service with:
    
    ```bash
    sudo systemctl enable inputplumber
    sudo systemctl start inputplumber
    ```

=== "Nix / NixOS"

    InputPlumber is available in nixpkgs for you to install. 

    If you are using NixOS, you can add the following to your `configuration.nix`:

    ```nix
    programs.inputplumber.enable = true;
    ```

    You can also install the package by adding InputPlumber
    to your package list:

    ```nix
    environment.systemPackages = with pkgs; [
      ...
      inputplumber
    ];
    ```

=== "Source / Tarball"

    InputPlumber requires the following system dependencies:
    
    - `libevdev`
    - `libiio`
    - `rust`
    
    To install the package, first visit the [release page](https://github.com/ShadowBlip/InputPlumber/releases) and download
    the `.tar.gz` package for your architecture.

    Once you have ensured your system dependencies are installed, you 
    can install InputPlumber from source with:
    
    ```bash
    make build
    sudo make install
    ```

    Then start the service with:
    
    ```bash
    sudo systemctl enable inputplumber
    sudo systemctl start inputplumber
    ```
