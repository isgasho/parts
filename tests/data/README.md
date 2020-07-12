# Data used by tests

## Generation

Empty files can be quickly created using `fallocate -l10M <file>`

See tool documentation for further details.

Note that neither cfdisk or parted allow specifying the UUID's manually,
so if these are ever regenerated tests will need updating.

## Files

### `test_parts`

An empty 10MiB file, created with gnu parted. Treated as if a 512 block size.

Expected Disk UUID: 062946B9-3113-4CC0-98DD-94649773E536
Expected Partition UUID: F3099835-0F4A-4D49-B012-7078CF1B4045

GPT label, one ext4-labeled partition, name "Test",
starting at 1MiB and ending at 9MiB, for a partition size of 8MiB.

The protective MBR start and end CHS is incorrect

### `test_parts_cf`

An empty 10MiB file, created with cfdisk. Treated as if a 512 block size.

Expected Disk UUID: A17875FB-1D86-EE4D-8DFE-E3E8ABBCD364
Expected Partition UUID: 97954376-2BB6-534B-A015-DF434A94ABA2

GPT label, one "Linux filesystem data", no name.
starting at 1MiB and ending at 9MiB, for a partition size of 8MiB.

`cfdisk` automatically aligns on 1MiB, unlike parted.

cfdisk generates random UUID's with an invalid version.
