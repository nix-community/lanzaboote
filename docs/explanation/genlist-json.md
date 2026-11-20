# The genlist.json file format

When the `--genlist-path` argument is provided to the `lzbt install` command, lanzaboote reads this JSON file to get the list of generations, instead of getting them from the `/nix/var/nix/profiles` directory.

This file is a JSONL file, where the [JSON schema](https://json-schema.org) of each line is:

```json
   {
     "$schema": "https://json-schema.org/draft/2020-12/schema",
     "$id": "https://lanzaboote.org/schemas/genlist-entry.json",
     "title": "GenlistEntry",
     "description": "A single entry of a generations list file. The generations list file is a JSONL file where each line describes a single generation.",
     "type": "object",
     "required": ["version", "outpath"],
     "properties": {
       "version": {
         "description": "The generation index",
         "type": "integer",
         "minimum": 0
       },
       "build_time": {
         "description": "The generation build time as a Unix timestamp",
         "type": ["integer", "null"],
         "minimum": 0
       },
       "outpath": {
         "description": "The generation Nix outpath",
         "type": "string",
         "format": "path"
       },
       "label": {
         "description": "An arbitrary label added to the boot entry",
         "type": "string",
         "default": ""
       }
     },
     "additionalProperties": false
   }
 ```

Moreover, lines must be ordered from the smallest to the highest version number. These versions number must be unique. The `label` must not contain `\n`.

A file can be validated by the `lzbt validate-genlist genlist.json` command.
