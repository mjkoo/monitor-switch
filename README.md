# monitor-switch

Set or toggle multiple monitor's input sources via DDC/CI

Based off of the excellent [ddcset](https://github.com/arcnmx/ddcset-rs/) and [ddc_hi](https://github.com/arcnmx/ddc-hi-rs) by arcnmx

## Motivation

This could have been done with some scripting over various utilities, but wanted a tool that could be triggered via hotkey on both Linux and Windows without much headache

The toggle command below is the actual command I have bound to scroll lock together with a USB switch to act as a sort of poor-man's KVM switch.

## Example Usage

Set all connected displays to DisplayPort 1

`monitor-switch set DisplayPort1`

Set all connected displays to HDMI 1

`monitor-switch set Hdmi1`

Toggle all connected displays from a given manufacturer between DisplayPort 1 and HDMI 1

`monitor-switch -g BNQ toggle DisplayPort1 Hdmi1`
