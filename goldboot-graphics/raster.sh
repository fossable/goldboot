#!/bin/bash

# Generate logo rasters
for height in 256 512 1024 2048 4096; do
	inkscape --export-type png -h ${height} -o output/logo-${height}.png output/logo.svg
	inkscape --export-type png -h ${height} -o output/logo-bg-${height}.png output/logo-bg.svg
done

# Generate icon rasters
for height in 32 64 128 256 512 1024 2048; do
	inkscape --export-type png -h ${height} -o output/icon-${height}.png output/icon.svg
	inkscape --export-type png -h ${height} -o output/icon-bg-${height}.png output/icon-bg.svg
done
