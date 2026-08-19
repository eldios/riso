# Builds from an already-compiled binary, so the same artifact ships in every
# package. Run `cargo build --release` first.
Name:           riso
Version:        0.3.0
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
