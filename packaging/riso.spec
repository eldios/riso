# Builds from an already-compiled binary, so the same artifact ships in every
# package. Run `cargo build --release` first.
Name:           riso
Version:        0.4.0
Release:        1%{?dist}
Summary:        Ricing framework for Linux desktops

License:        MIT
URL:            https://github.com/eldios/riso

# Fetched at run time to install themes and plugins.
Requires:       git
Requires:       curl

%description
riso renders a theme into the configuration files a desktop reads. A theme is
data: a palette, some typography, optionally a wallpaper. Everything else is
generated, so adding support for an application does not make existing themes
incomplete.

%install
install -Dm755 %{_sourcedir}/riso %{buildroot}%{_bindir}/riso
install -Dm644 %{_sourcedir}/riso.1 %{buildroot}%{_mandir}/man1/riso.1
install -Dm644 %{_sourcedir}/LICENSE %{buildroot}%{_datadir}/licenses/%{name}/LICENSE
install -Dm644 %{_sourcedir}/NOTICE %{buildroot}%{_datadir}/doc/%{name}/NOTICE
install -Dm644 %{_sourcedir}/README.md %{buildroot}%{_datadir}/doc/%{name}/README.md

%files
%{_bindir}/riso
%{_mandir}/man1/riso.1*
%license %{_datadir}/licenses/%{name}/LICENSE
%doc %{_datadir}/doc/%{name}/NOTICE
%doc %{_datadir}/doc/%{name}/README.md

%changelog
* Thu Aug 20 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.4.0-1
- the default catalog moves to catalog.riso.re, decoupled from the hosting
- the theme gallery lives at catalog.riso.re, the project at riso.re

* Thu Aug 20 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.3.4-1
- themes without colors.toml derive their palette from alacritty.toml
- riso-bin joins riso on the AUR, both published by the release CI
- cava's source = auto no longer needs --trust

* Thu Aug 20 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.3.3-1
- theme update narrates progress per theme and overall, -q for scripts
- nonzero exit when an update fails or is refused
- flake reads its version from Cargo.toml and can run tests sandboxed

* Thu Aug 20 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.3.2-1
- riso config: config.toml with the default output format and an
  omarchy-themes switch to drop Omarchy's directories from the search path
- The built-in fallback theme slims to one small wallpaper

* Wed Aug 19 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.3.1-1
- Component-first CLI: theme, backgrounds, plugin, dev, with -o json/yaml
- Picking is a mode: theme set --gui (carousel) and --tui (terminal picker)
- A boot keeps the wallpaper it finds instead of advancing past it

* Tue Aug 18 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.3.0-1
- A theme returns to the wallpaper it was last set to

* Tue Aug 18 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.2.1-1
- Theme switches on Omarchy repaint the wallpaper and run the retint hooks

* Tue Aug 18 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.2.0-1
- Noctalia palette joins the builtin templates
- Theme validation only flags directives in statement position
- The rpm package is named riso

* Sun Aug 16 2026 Emanuele Calo <emanuele.lele.calo@gmail.com> - 0.1.0-1
- First packaged release
