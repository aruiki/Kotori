@echo off
rem Uninstall Kotori. Double-click this file; Windows asks for administrator rights.
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"%~dp0uninstall.ps1\"','-Pause'"
