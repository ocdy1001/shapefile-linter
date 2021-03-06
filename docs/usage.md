# shapefile-linter

shapefile-linter is a program that can convert and lint shapefiles and kml files.
The main feature is the conversion of shapefiles/kml to custom binairy files.
The goal of these files is to compress specific types of shapefiles like geographic heightmaps to a
denser file that we use in `uu-uce`.
It can also do things like print information about a shapefile and lint a shapefile while compressing it.

## general usage

shapefile-linter is cli tool. It means command line interface, meaning you interact with it in the terminal.
You use it by giving it flags and arguments, like `shapefile-linter --help`

### input files

Except for the `shapefile-linter --help` command, you always need to give the input files as the first argument.
For example `shapefile-linter data/heightmap.shp`.
It can be multiple files like this: `shapefile-linter data/a.shp data/b.shp`
You can ofcourse use patterns supported by your shell, like `bash` or `zsh`.
For example get all shapefiles from a directory: `shapefile-linter data/*.shp`.
Or get all shapefiles that contain `al` in the name in all sub directories:
`shapefile-linter data/**/*al*.shp`

### output files

Using the output flag you can provide the name of the output file. The standard output name is `outp`.
Using `shapefile-linter somefile.shp --mode someNonExistantMode --output hello` we tell shapefile-linter
that it should save the output as the file `hello`.
This does not follow for commands that output chunks.
They have fixed names as the uu-uce app expects certain files.

### ft

The argument `--ft` let you choose what filetype you are inputting if there are multiple choices.
For example, `--ft shape` and `--ft kml` are used in some commands.

### shapeinfo

Using `shapefile-linter file.shp --mode shapeinfo` you can print out what is inside the shapefile.
It will print out how much of each shape type is in there and how many parts and rings they have.

### mergeheight

The command `shapefile-linter *.shp --mode mergeheight --ft shape` will take all shapefiles and assume they are heightmaps.
It will compress them and store them into one big custom file.
It will work for kml.

### lintheight

Using `shapefile-linter file.shp --mode lintheight --ft shapefile` you can see if there are wrong heightlines and some stats about them.
This does also work with kml, see the `ft` section.

### chunkify

Chunkify takes a merged file from the command `mergeheight`.
Works for shapefile and kml.
You can give the amount of cuts with the cuts flag.
N cuts mean you end up with N^2 chunks.
Example: `shapefile-linter file.kml --mode chunkify --ft kml`.
For more tuning, you can use the flags `cuts_multi`, `levels` and `mods`.
cuts_multi will set the multiplier of the cuts. For example, when cuts is 2 and the multi is 4, each level the cuts will multiply by 4 creating the sequence `2,8,32,...`.
Levels will set the amount of LOD levels it will create.
Mods takes a list of levels numbers. In other words a list of N numbers where N is the levels parameter.
This will set the heightline modulo of each level.
Only heightlines with z levels in the set `{ z | z % modulo = 0 }` will be included at every LOD level.

### polygonz

The command `shapefile-linter file.shp --mode polygonz` will take the shapefile and assume it is an shapefile
containing only PolygonZ types. It will trow away the w coordinate and store compressed shapes into a custom file.

### triangulate

triangulate will take a shapefile, turn it into polygons and triangulate them.
These triangles will be compressed and saved to disk.

### height

The command `shapefile-linter file.shp --mode height --ft shape` will take the shapefile and assume it only contains
PolylineZ's. It will store them compressed and efficiently in a custom file. Every ShapeZ will have a single z value.
It is assumed that all points in a PolylineZ have the same z value. If not, the shape is not included and a warning
is givin. Will work for kml and shapefile.

### xmltree

Prints out the xml start tags in an indented tree form.
Example: `shapefile-linter file.xml --mode xmltree`

### xmltags

Prints out every tag with its count.
Example: `shapefile-linter file.xml --mode xmltags`

### geopolys

geopolys takes a list of kml files, merges them together, extract the polygons, triangulates them, cuts them into chunks, compresses those chunks and then writes them out.
You can give the amount of cuts with the cuts flag.
N cuts mean you end up with N^2 chunks.
Example: `shapefile-linter dir/**/*.kml --mode geopolys --cuts 8`

### geolines

geolines will take a list of kml files, merges them together, extract the lines, cuts them into chunks, match the styles, compress them and writes them out.
Example: `shapefile-linter lines.kml --mode geolines --cuts 8`

### check-tag-child

A debug check for XML files, takes a list of XML files and checks if a tag always has a child tag.
Example: `shapefile-linter test.xml --mode check-tag-child --tag0 outer --tag1 inner`

### check-nonempty-tag

Takes a list of XML files and checks if a tag is always nonepty.
Example: `shapefile-linter test.xml --mode check-nonempty-tag --tag0 testtag`
