#!/usr/bin/env python3
"""Wrap a verified Windows release bundle in a per-user, transactional MSI."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import uuid
import xml.etree.ElementTree as xml

import package


HOST = "x86_64-pc-windows-msvc"
WIX_VERSION = "4.0.6"
UPGRADE_CODE = "AE76BF01-72DA-4A42-9928-C813C654CC86"
NAMESPACE = "http://wixtoolset.org/schemas/v4/wxs"
xml.register_namespace("", NAMESPACE)


def installer_version(value):
    # MSI has its own ordered, three-integer version, not SemVer prereleases.
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value):
        raise ValueError("installer version must be three canonical decimal integers")
    if any(number > limit for number, limit in zip(map(int, value.split(".")), (255, 255, 65535))):
        raise ValueError("installer version exceeds MSI's 255.255.65535 limit")
    return value


def identifier(kind, path):
    return kind + hashlib.sha256(path.encode("utf-8")).hexdigest()[:32]


def product_code(version):
    return str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), installer_version(version))).upper()


def author(bundle, version):
    """The manifest owns the complete payload; MSI owns only installation state."""
    version = installer_version(version)
    manifest = package.verify_manifest(bundle)
    if (manifest.get("schema") != "omega-release-bundle/1" or manifest.get("dirty") is not False
            or manifest.get("host") != HOST or "bin/omega.exe" not in manifest["files"]):
        raise ValueError("installer requires a clean Windows x64 release bundle")

    def element(parent, name, **attributes):
        return xml.SubElement(parent, f"{{{NAMESPACE}}}{name}", attributes)

    root = xml.Element(f"{{{NAMESPACE}}}Wix")
    product = element(root, "Package", Name=f"Omega {manifest['version']}", Manufacturer="CathedralOS",
                      Version=version, UpgradeCode=UPGRADE_CODE, ProductCode=product_code(version),
                      Scope="perUser", Language="1033", InstallerVersion="500")
    element(product, "MajorUpgrade", Schedule="afterInstallInitialize",
            DowngradeErrorMessage="A newer Omega installer is already installed.")
    element(product, "MediaTemplate", EmbedCab="yes")
    element(product, "Property", Id="ARPCOMMENTS", Value=f"Omega {manifest['version']} ({manifest['commit']})")
    local = element(product, "StandardDirectory", Id="LocalAppDataFolder")
    programs = element(local, "Directory", Id="OmegaPrograms", Name="Programs")
    install = element(programs, "Directory", Id="INSTALLFOLDER", Name="Omega")
    directories = {".": install}
    feature = element(product, "Feature", Id="Omega", Title="Omega", Level="1")

    def directory(path):
        key = path.as_posix()
        if key not in directories:
            directories[key] = element(directory(path.parent), "Directory",
                                       Id=identifier("Directory", key), Name=path.name)
        return directories[key]

    def component(parent, key):
        name = identifier("Component", key)
        result = element(parent, "Component", Id=name,
                         Guid=str(uuid.uuid5(uuid.UUID(UPGRADE_CODE), key)).upper())
        # User-profile files need a per-user registry key path (ICE38), rather
        # than a file that would confuse installation state across users.
        element(result, "RegistryValue", Root="HKCU", Key="Software\\CathedralOS\\Omega\\Installer",
                Name=name, Type="integer", Value="1", KeyPath="yes")
        element(feature, "ComponentRef", Id=name)
        return result

    for relative in sorted([*manifest["files"], "manifest.json"]):
        path = Path(relative)
        parent = directory(path.parent)
        owner = component(parent, relative)
        element(owner, "File", Id=identifier("File", relative), Source=str((bundle / path).resolve()))
    # Remove only empty authored directories. Never recursively delete user data.
    for key, parent in sorted(directories.items()):
        owner = component(parent, "directory:" + key)
        element(owner, "RemoveFolder", Id=identifier("Remove", key), On="uninstall")
    owner = component(install, "environment:PATH")
    element(owner, "Environment", Id="OmegaPath", Name="PATH", Value="[INSTALLFOLDER]bin",
            Action="set", Part="last", System="no", Permanent="no")
    xml.indent(root)
    return xml.tostring(root, encoding="utf-8", xml_declaration=True)


def build(bundle, version, output, wix):
    document = author(bundle, version)
    if os.name != "nt":
        raise ValueError("building an MSI requires a Windows host; authoring/tests are portable")
    environment = dict(os.environ, DOTNET_ROLL_FORWARD="Major")
    actual = package.run([wix, "--version"], environment=environment).strip().split("+")[0]
    if actual != WIX_VERSION:
        raise ValueError(f"expected WiX {WIX_VERSION}, found {actual}")
    output.mkdir(parents=True, exist_ok=False)
    source = output / "installer.wxs"
    source.write_bytes(document)
    manifest = package.verify_manifest(bundle)
    installer = output / f"omega-{manifest['version']}-{HOST}.msi"
    package.run([wix, "build", source, "-arch", "x64", "-o", installer], environment=environment)
    installer.with_suffix(".msi.sha256").write_text(
        f"{hashlib.sha256(installer.read_bytes()).hexdigest()}  {installer.name}\n", encoding="ascii")
    return installer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True, help="verified, extracted Windows bundle")
    parser.add_argument("--installer-version", required=True)
    parser.add_argument("--output", type=Path, required=True, help="new output directory")
    parser.add_argument("--wix", type=Path, required=True)
    options = parser.parse_args()
    print(build(options.bundle, options.installer_version, options.output, options.wix.resolve()))


if __name__ == "__main__":
    main()
