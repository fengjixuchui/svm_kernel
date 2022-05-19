  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:

  let 
  vscodeEnv = pkgs.callPackage ./nix/codium/vscodeEnv.nix {
    extensionsFromVscodeMarketplace = pkgs.vscode-utils.extensionsFromVscodeMarketplace;
    vscodeDefault = pkgs.vscodium;
  };

  ipxe = pkgs.callPackage ./nix/ipxe { };

  myvscode = vscodeEnv {
   vscodeBaseDir = toString ./.vscode;
   nixExtensions =  pkgs.vscode-utils.extensionsFromVscodeMarketplace [
      {
        name = "language-x86-64-assembly";
        publisher = "13xforever";
        version = "3.0.0";
        sha256 = "sha256-wIsY6Fuhs676EH8rSz4fTHemVhOe5Se9SY3Q9iAqr1M=";
      }
      {
        name = "vscode-coverage-gutters";
        publisher = "ryanluker";
        version = "2.8.2";
        sha256 = "sha256-gMzFI0Z9b7I7MH9v/UC7dXCqllmXcqHVJU7xMozmMJc=";
      }
      {
        name = "llvm";
        publisher = "rreverser";
        version = "0.1.1";
        sha256 = "sha256-MPY854kj34ijQqAZQCSvdszanBPYzxx1D7m+3b+DqGQ=";
      }
    ] ++ (with pkgs.vscode-extensions;  [
      yzhang.markdown-all-in-one
      timonwong.shellcheck
      tamasfe.even-better-toml
      serayuzgur.crates
      jnoortheen.nix-ide
      matklad.rust-analyzer
      vadimcn.vscode-lldb
      #ms-python.python # Broken on nixos unstable
    ]);
    settings = {
      window.menuBarVisibility = "toggle";
      window.zoomLevel = 0;
      editor.fontSize = 16;
      terminal.integrated.fontSize = 16;
      lldb.displayFormat = "hex";
      breadcrumbs.enabled = false;
      files.associations."*.s" = "asm-intel-x86-generic";
      rust-analyzer.inlayHints.parameterHints = false;
      workbench.colorCustomizations = {
        statusBar.background = "#1A1A1A";
        statusBar.noFolderBackground = "#212121";
        statusBar.debuggingBackground = "#263238";
      };
    };

    keybindings = [
      {
          key = "f6";
          command = "workbench.action.tasks.runTask";
          args = "rust: cargo run";
      }
      {
          key = "f4";
          command = "workbench.action.tasks.runTask";
          args = "Debug kernel";
      }
    ];
  };

  myipxe = (ipxe.override {
        # Script fixes race condition where router dns replies first
        # and pxe boot server second
        embedScript = pkgs.writeText "ipxe_script" ''
          #!ipxe
          dhcp
          autoboot
          shell
        '';
      });
  in 
  pkgs.mkShell rec {
    buildInputs = with pkgs; [
      evcxr # rust repl
      cargo-tarpaulin # for code coverage
      myvscode
      rust-analyzer
      zlib.out
      rustup
      xorriso
      dhcp
      myipxe
      grub2
      qemu
      entr # bash file change detector
      #glibc.dev # Creates problems with tracy
      netcat-gnu
      git-extras
      python3
    ] ++ (with pkgs.python39Packages; [
      pyelftools
      intervaltree
    ]) ++ (with pkgs.llvmPackages_latest; [
      lld
      bintools
      llvm
    ]);
    IPXE =  myipxe;
    RUSTC_VERSION = pkgs.lib.readFile ./rust-toolchain;
    # https://github.com/rust-lang/rust-bindgen#environment-variables
    LIBCLANG_PATH= pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
    RUSTFLAGS = (builtins.map (a: ''-L ${a}/lib'') [
    ]);
    BINDGEN_EXTRA_CLANG_ARGS = 
    # Includes with normal include path
    (builtins.map (a: ''-I"${a}/include"'') [
      pkgs.glibc.dev 
    ])
    # Includes with special directory paths
    ++ [
      ''-I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
    ];
    HISTFILE=toString ./.history;
    shellHook = ''
      export PATH=$PATH:~/.cargo/bin
      export PATH=$PATH:~/.rustup/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
      '';
  }
