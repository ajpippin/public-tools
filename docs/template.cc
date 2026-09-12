/*= template.cc ==============================================================*\
|*									      *|
|*  Entry point into template.						      *|
|*									      *|
|* @author:    Andy Pippin  <pippin@ajpippin.us>			      *|
|* @version:   $Revision: 1.10 $  $Date: 2004/08/22 01:06:17 $		      *|
|* @requires:								      *|
|*									      *|
\*============================================================================*/


//-[ Identity ]-----------------------------------------------------------------

#define __TEMPLATE_CC__
static char acSrcIdent[] =
    "$Id: template.cc,v 1.10 2004/08/22 01:06:17 pippin Exp $";


//-[ Includes ]-----------------------------------------------------------------

#include  <ctype.h>
#include  <errno.h>
#include  <getopt.h>
#include  <libgen.h>
#include  <stdio.h>
#include  <stdlib.h>
#include  <string.h>
#include  <sys/types.h>
#include  <sys/stat.h>
#include  <time.h>
#include  <unistd.h>


//-[ Typedefs/Structures/Enums ]------------------------------------------------

typedef struct _substitute
{
    char*  key;		// The key.
    char*  val;		// The substitute.
    bool   ign;		// Ignore case?
}
  SUBSTITUTE;


//-[ Constants ]----------------------------------------------------------------

static int const  DEFAULT_Verbosity = 1;
static int const  MAX_Substitutes   = 50;


//-[ Variables ]----------------------------------------------------------------

// Command line options.
//
struct option  optionList[] = 
{
    { "list",		optional_argument,	NULL,	'l' },
    { "show",		optional_argument,	NULL,	 2  },
    { "template",	required_argument,	NULL,	't' },

    { "help",		optional_argument,	NULL,	'h' },
    { "quiet",		no_argument,		NULL,	'q' },
    { "usage",		no_argument,		NULL,	 1  },
    { "version",	no_argument,		NULL,	'V' },
    { "verbose",	optional_argument,	NULL,	'v' },

    { NULL,		no_argument,		NULL,	 0 }
};

static char const  shortOptions[] = "hqt:vV";


// Program name.
//
static char*  programName;


// Global verbose level.
//
#ifdef  __cplusplus
namespace  global
{
#endif

    int verboseLevel = DEFAULT_Verbosity;

#ifdef  __cplusplus
};
#endif


// Substitutions.
//
static SUBSTITUTE  substitute[ MAX_Substitutes ];


//-[ Prototypes ]---------------------------------------------------------------

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Invokes user-defined code (like, duh!).
 *
 * @param   numArgs  Number of arguments on the command line.
 * @param   args     Arguments from the command line.
 * @param   envs     Environmental variables.
 * @return  Exit code.
 */
	int 
main(
    int	   numArgs,
    char*  args[],
    char*  envs[]
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Display correct usage (synopsis) of the application.
 *
 * @param   output  Stream to display output.
 * @param   prefix  What to put before each line.
 * @param   argv0   Application name.
 */
	static void
printUsage(
    FILE*  output,
    char*  prefix,
    char*  argv0
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Display complete description of the options.
 *
 * @param   topic   Topic of interest.
 * @param   output  Stream to display output.
 * @param   prefix  What to put before each line.
 * @param   argv0   Application name.
 */
	static void
printHelp(
    char*  topic,
    FILE*  output,
    char*  prefix,
    char*  argv0
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Writes the application name and version to stdout.
 *
 * @param   output  Stream to display output.
 * @param   prefix  What to put before each line.
 * @param   argv0   Application name.
 */
	static void
printVersion(
    FILE*  output,
    char*  prefix,
    char*  argv0
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Compares two strings together up to the length of the shortest.
 *
 * @param   str1    String #1.
 * @param   str2    String #2.
 */
	static bool
isEqual(
    const char*  str1,
    const char*  str2
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Creates a file from a template.
 *
 * @param   targetFilename   The name of the file to be created.
 * @param   templateName     The name of the template to use.
 */

	static void
createFileFromTemplate(
    char*  targetFilename,
    char*  templateName =NULL
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Get the extension of the target filename.  The result may be a
 *  colon-separated list of templates.
 *
 * @param   targetFilename  The name of the target file.
 * @return  The template name to use.
 */
	static const char*
getExtension(
    char* targetFilename
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Get the fully-qualified name of the template to use.
 *
 * @param   searchDirs    A colon-separated list of directories to search.
 * @param   templateName  The name of the template.
 * @return  The name of the template, or NULL if not found.
 */

	static char*
getTemplate(
    char*  searchDirs,
    char*  templateName
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Get the list of directories that might contain the template.  The current
 *  directory (".") will always be at the end of the list.
 *
 * @return  A colon separated list of directories to search.
 */

	static char*
getSearchDirectories(
    void
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Opens the two files and invokes the creator.
 *
 * @param   templateFilename  Name of the template.
 * @param   targetFilename    Name of the file to generate.
 */
	static void
generateFile(
    char*  templateFilename,
    char*  targetFilename
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Reads in the template, substitutes for keywords, and writes to the target.
 *
 * @param   templateFile  Input file (the template).
 * @param   targetFile    Output file (the target).
 */
	static void
createTargetFile(
    FILE* templateFile,
    FILE* targetFile
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Tries to open the destination file.  If the file already exists, it
 *  will ask for confirmation.
 *
 * @param   targetFilename    Name of the file to generate.
 */
	static FILE*
openTargetFile(
    char* targetFilename
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Get a substitute.
 *
 * @param   tag  The possible tag.
 * @return  The substitute for the tag, or the tag itself if unknown.
 */
	static char*
getSubstitute(
    char*  tag
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Make the substitutes.
 *
 * @param   templateFilename  Name of the template.
 * @param   targetFilename    Name of the file to generate.
 */
	static void
makeSubstitutes(
    const char*  templateFilename,
    const char*  targetFilename
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  "Fix" a null string.
 *
 * @param   string  String in question.
 * @param   delim   Delimiter.
 * @return  Either NULL or the string surrounded by the delimiter.
 */
	static char*
fixNull(
    char*  string,
    char*  delim  = (char*)"\""
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  List all the known templates.
 *
 * @return  Nothing.
 */
	static void
listTemplates(
    void
);


//==[ Implementation ]==========================================================

	static bool
isEqual(
    const char*  str1,
    const char*  str2
){
    bool  result = false;
    int   len, len1, len2;

    if( (NULL == str1) && (NULL == str2) )
    {
        result = true;
    }

    else if( (NULL != str1) && (NULL != str2) )
    {
        len1 = strlen( str1 );
        len2 = strlen( str2 );
	len  = (len1 < len2) ? len1 : len2;

        result = (0 == strncasecmp( str1, str2, len )) ? true : false;
    }

    return( result );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
printVersion(
    FILE*  output,
    char*  prefix,
    char*  argv0
){
    // Local variables.
    //
    static char  rcsVersion[] = "$Revision: 1.10 $";
    static char  rcsDate[]    = "$Date: 2004/08/22 01:06:17 $";
    static char  rcsAuthor[]  = "$Author: pippin $";

    char   versBuffer[ 80 ];			// Result.
    char   buffer[ 256 ]; 			// Working tool.
    char*  pBegin;				// Working tool.
    char*  pEnd;				// Working tool.
    char*  pSrc;				// Working tool.
    char*  pDst;				// Working tool.


    // Fix parameters.
    //
    argv0  = (NULL != argv0)  ? argv0  : programName;
    prefix = (NULL != prefix) ? prefix : (char*)"";
    output = (NULL != output) ? output : stderr;


    // Get version.
    //
    strncpy( buffer, rcsVersion, sizeof(buffer) );
    if( NULL != (pBegin = strchr( buffer, ':' )) )
    {
        pBegin += 2;
	if( NULL != (pEnd = strchr( pBegin, '$' )) )
	{
	    *(--pEnd) = '\0';
	    strncat( versBuffer, " v", sizeof(versBuffer)-1 );
	    strncat( versBuffer, pBegin, sizeof(versBuffer)-1 );
	}
    }

    // Get date.
    //
    strncpy( buffer, rcsDate, sizeof(buffer) );
    if( NULL != (pSrc = strchr( buffer, ':' )) )
    {
        pSrc += 2;
	pDst = versBuffer + strlen( versBuffer );

	*(pDst++) = ' ';
	*(pDst++) = *(pSrc++);		// C.../../..
	*(pDst++) = *(pSrc++);		// .C../../..
	*(pDst++) = *(pSrc++);		// ..Y./../..
	*(pDst++) = *(pSrc++);		// ...Y/../..
	pSrc++;
	*(pDst++) = *(pSrc++);		// ..../M./..
	*(pDst++) = *(pSrc++);		// ..../.M/..
	pSrc++;
	*(pDst++) = *(pSrc++);		// ..../../D.
	*(pDst++) = *(pSrc++);		// ..../../.D
	*pDst = '\0';
    }

    // Get author.
    //
    strncpy( buffer, rcsAuthor, sizeof(buffer) );
    if( NULL != (pBegin = strchr( buffer, ':' )) )
    {
        pBegin += 2;
	if( NULL != (pEnd = strchr( pBegin, '$' )) )
	{
	    *(--pEnd) = '\0';
	    strncat( versBuffer, "/", sizeof(versBuffer)-1 );
	    strncat( versBuffer, pBegin, sizeof(versBuffer)-1 );
	}
    }

    fprintf( output, "%s%s %s\n", prefix, argv0, versBuffer );
    fflush( output );

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void 
printUsage(
    FILE*  output,
    char*  prefix,
    char*  argv0
){
    // Fix parameters.
    //
    output = (NULL != output) ? output : stdout;
    prefix = (NULL != prefix) ? prefix : (char*)"";
    argv0  = (NULL != argv0)  ? argv0  : programName;

    fprintf( output, "%sUsage: %s [-t template] [-lshqVv]\n", prefix, argv0 );
    fflush( output );

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
printHelp(
    char*  topic,
    FILE*  output,
    char*  prefix,
    char*  argv0
){
    // Local variables.
    //
    char  isNull;
    char  isAll;

    // Set flags.
    //
    isNull = (NULL == topic) ? 1 : 0;
    if( ! isNull )
	isAll = (0 == strcasecmp( "all", topic )) ? 1 : 0;


    // Fix parameters.
    //
    topic  = (NULL != topic)  ? topic  : (char*)"";
    output = (NULL != output) ? output : stdout;
    prefix = (NULL != prefix) ? prefix : (char*)"";
    argv0  = (NULL != argv0)  ? argv0  : programName;


    // *** Now start displaying help. ***

    // Description
    //
    if( isNull || isAll || isEqual( topic, "desc" ) )
    {
        if( isNull || isAll )
	    fprintf( output, "\n%s%s\n\n",
	             "-- Description -------------------------", 
		     "----------------------------------------" );

        fprintf( output, "%s\"%s\" ...\n",
	    prefix,
	    argv0 );
        fprintf( output, "%s\n", prefix );
    }

    // Usage
    //
    if( isNull || isAll || isEqual( topic, "usage" ) )
    {
        if( isNull || isAll )
	    fprintf( output, "\n%s%s\n\n",
		    "-- Usage -------------------------------", 
		    "----------------------------------------" );

	printUsage( output, prefix, argv0 );
        fprintf( output, "%s\n", prefix );
    }

    // Options
    //
    if( isNull || isAll || isEqual( topic, "option" ) )
    {
        if( isNull || isAll )
	    fprintf( output, "\n%s%s\n\n",
		    "-- Options -----------------------------", 
		    "----------------------------------------" );

        fprintf( output,
"%s  If a long option shows an argument as mandatory, then it is mandatory\n\
%s  for the equivalent short option also.  Similarly for optional arguments.\n",
	    prefix,
	    prefix );
        fprintf( output, "%s\n", prefix );

	// Main operation.
	//
        fprintf( output, "%s  ** Main Operation Modes **\n", prefix );
        fprintf( output, 
"%s        --list             List all the known templates.\n\
%s        --show             Show all the substitutions.\n\
%s    -t, --template=file    Use a particular filename as a template.\n",
		prefix,
		prefix,
		prefix );
        fprintf( output, "%s\n", prefix );


	// Informative/Debug options.
	//
        fprintf( output, "%s  ** Informative Output **\n", prefix );
        fprintf( output, 
"%s    -h, --help[=topic]     Display this help message or details on a topic.\n\
%s        --usage            Display synopsis of command line arguments.\n\
%s    -v, --verbose[=level]  Set verbosity (def = %d)\n\
%s    -q, --quiet            Set verbosity to 0.\n\
%s    -V, --version          Print version to stdout and exit.\n",
		prefix,
		prefix,
		prefix, DEFAULT_Verbosity,
		prefix,
		prefix );
        fprintf( output, "%s\n", prefix );
    }


    // Topics
    //
    if( isAll || isEqual( topic, "topic" ) )
    {
        if( isNull || isAll )
	    fprintf( output, "\n%s%s\n\n",
		    "-- Topics ------------------------------", 
		    "----------------------------------------" );

        fprintf( output, 
"%s  all      Display this help message or details on a topic.\n\
%s  desc     Description of this command.\n\
%s  topic    Help topics.\n\
%s  usage    Synopsis of command line arguments.\n\
%s  option   Description of command line arguments.\n\
%s  keyword  Listing of words substituted.\n",
		prefix,
		prefix,
		prefix,
		prefix,
		prefix,
		prefix );
        fprintf( output, "%s\n", prefix );
    }

    // Keywords
    //
    if( isEqual( topic, "keyword" ) )
    {
	fprintf( output, "\n%s%s\n\n",
		    "-- Keywords ----------------------------", 
		    "----------------------------------------" );
        fprintf( output,
"%s    Keywords must be delimited with two percent signs (%%%%).  For example, the\n\
%s  string \"%%%%time%%%%\" will be replaced with a string similiar to \"12:34:56\".\n\
%s\n\
%s    If the target filename was \"Target.c\", then the keywords and their\n\
%s  substitutions are:\n", prefix, prefix, prefix, prefix, prefix );

	
        fprintf( output, "%s\n", prefix );
	fprintf( output, "%s  Keyword             Case   Substitute Value\n", prefix );
	fprintf( output, "%s -------------------  ----  -------------------------------------------------\n", prefix );
	makeSubstitutes( "Template", "./this/is/a/path/Target.c" );


	int sub;
	char  keyValue[ 32 ];
	char  valValue[ 48 ];

	for( sub=0; NULL != substitute[ sub ].key; ++sub )
	{
	    strcpy( keyValue, "%%" );
	    strncat( keyValue, substitute[sub].key, sizeof(keyValue)-1 );
	    strncat( keyValue, "%%", sizeof(keyValue)-1 );

	    // Truncate value.
	    strncpy( valValue, substitute[sub].val, sizeof(valValue) );
	    valValue[ sizeof(valValue)-1 ] = '\0';
	    if( 0 != strcmp( valValue, substitute[sub].val ) )
	    {
	        valValue[ sizeof(valValue)-5 ] = ' ';
	        valValue[ sizeof(valValue)-4 ] = '.';
	        valValue[ sizeof(valValue)-3 ] = '.';
	        valValue[ sizeof(valValue)-2 ] = '.';
	    }

	    fprintf( output, "%s  %-18.18s  %s   %s\n", prefix,
	    keyValue,
	    (substitute[sub].ign) ? "Ign " : "    ",
	    valValue );
	}
        fprintf( output, "%s\n", prefix );
    }

    fflush( output );
    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	int
main(
    int    numArgs,
    char*  args[],
    char*  envs[]
){
    // Local variables.
    //
    int    exitCode = 0;			// Exit code.
    int    shortOption = 0;			// Short option value.
    int    longOption = 0;			// Long option value.
    char   cmdError = 0;			// Command line parsing error.
    char*  templateName = NULL;			// Specify a template.


    // Initialize.
    //
    programName = basename( args[ 0 ] );


    // Parse the command line arguments.
    //
    while( EOF != (shortOption = getopt_long( numArgs, args, shortOptions,
			optionList, &longOption )) )
    {
	switch( shortOption )
	{
          // *** LIST TEMPLATES ***
          //
          case  'l':
	    listTemplates();
	    return(0);
	    break;

          // *** SPECIFY TEMPLATE ***
          //
          case  't':
	    templateName = optarg;
	    break;

          // *** SHOW SUBSTITUTIONS ***
          //
          case  2:
	    makeSubstitutes( NULL, NULL );
	    if( NULL != optarg )
	    {
	        printf( "  \"%s\"\t\t--> \"%s\"\n",
				optarg, getSubstitute( optarg ) );
	    }
	    else
	    {
	        for( int sub=0; NULL != substitute[sub].key; ++sub )
		    printf( "  \"%s\"\t\t--> \"%s\"%s\n",
			substitute[sub].key,
			substitute[sub].val,
			(substitute[sub].ign) ? "   (Ignore case)" : "" );
	    }
	    exitCode = 1;
	    break;


	  // *** HELP ***
	  //
	  case  'h':
	    printHelp( optarg, NULL, NULL, NULL );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** USAGE ***
          //
          case  1:
	    printUsage( NULL, NULL, NULL );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** VERBOSE ***
          //
          case  'v':
            if( NULL == optarg )
#ifdef  __cplusplus
                global::verboseLevel++;
#else
                verboseLevel++;
#endif
            else
#ifdef  __cplusplus
                global::verboseLevel = atoi( optarg );
#else
                verboseLevel = atoi( optarg );
#endif
            break;


          // *** QUIET ***
          //
          case  'q':
#ifdef  __cplusplus
	    global::verboseLevel = 0;
#else
	    verboseLevel = 0;
#endif
            break;


	  // *** VERSION ***
	  //
	  case  'V':
	    printVersion( NULL, NULL, NULL );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***



	  // *** ERROR ***
	  //
	  case  '?':
	  default:
	    cmdError = 1;
	    break;
	}
    }


    // Bail.
    //
    if( 0 != exitCode )
    {
        return( exitCode );
    }


    // Make sure we have the right number of items to work with.
    //
    else if( 0 == (numArgs - optind) )
    {
        cmdError = 1;
    }


    // Print error message.
    //
    if( cmdError )
    {
	printUsage( NULL, NULL, NULL );
	exitCode = 1;
    }

    // Do the work.
    //
    else
    {
	// Loop through items.
	//
	for( args = &(args[optind]); NULL != *args; ++args )
	{
	    createFileFromTemplate( *args, templateName );
	}
    }


    // Exit.
    //
    return( exitCode );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	char** 
splitString(
    char* str,
    char delim[]
) {
    if( NULL == str ) {
        return NULL;
    } else {
        int init_size = strlen(str);

	char *ptr = strtok(str, delim);

	while(ptr != NULL) {
	    printf("'%s'\n", ptr);
	    ptr = strtok(NULL, delim);
	}

	return (char**)&str;
    }
}

	static void
listTemplates(
    void
){
    static char* searchDirectories = getSearchDirectories();
    printf("Search directories: \"%s\"\n", searchDirectories);

    char** directories = splitString(searchDirectories, (char*)":");
    for(char* dir = *directories; NULL != dir; ++dir) {
	printf("'%s'\n", dir);
    }
}



//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
createFileFromTemplate(
    char*  targetFilename,
    char*  templateName
){
    char*  extension = NULL;
    char*  nextExtension = NULL;
    char*  templateFilename = NULL;

    if( NULL != targetFilename )
    {
	if( (NULL != templateName)
	 && (NULL != (templateFilename = getTemplate(
				    getSearchDirectories(), templateName ))) )
	{
	    generateFile( templateFilename, targetFilename );
	  
	}

        else if( NULL != (templateFilename = getTemplate(
				    getSearchDirectories(), targetFilename )) )
	{
	    generateFile( templateFilename, targetFilename );
	}


	else if( NULL != (extension = (char*)getExtension( targetFilename )) )
	{
	    nextExtension = index( extension, ':' );

	    while( NULL != extension )
	    {
		// If we have multiple, then terminate this one and go to
		// the next.
		//
		if( NULL != nextExtension )
		{
		    *(nextExtension++) = '\0';
		}

		templateFilename = getTemplate( getSearchDirectories(), 
								    extension );

		if( (NULL == templateFilename)
		 && (0 < global::verboseLevel) )
		{
		    fprintf( stderr,
			    "Couldn't find a suitable template for: \"%s\"\n",
			    targetFilename );
		}

		else
		{
		    generateFile( templateFilename, targetFilename );
		}

		// Do the next one.
		//
		extension = nextExtension;
		if( NULL != nextExtension )
		{
		    nextExtension = index( nextExtension, ':' );
		}
	    }
	}
    }

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static const char*
getExtension(
    char* targetFilename
){
    const char*  extension = NULL;

    if( NULL != targetFilename )
    {
	// See if there is a "." something, and return the something.
	//
        if( NULL != (extension = strrchr( (const char*)targetFilename, '.' )) )
	{
	    extension++;

	    if( 0 == strcmp( extension, "cchh" ) )
	        extension = "cc:hh";
	    else if( 0 == strcmp( extension, "ch" ) )
	        extension = "c:h";
	}

	// Otherwise, just return the filename.
	//
	else
	{
	    extension = targetFilename;
	}
    }

    return( extension );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static char*
getTemplate(
    char*  searchDirs,
    char*  templateName
){
    // Variables
    //
    char*  result = NULL;			// Template name, or NULL.
    char*  search = NULL;			// Current dir to search.
    char*  nextDir = NULL;			// Next dir to search.
    struct stat  statBuffer;			// Check for existance.
    static char  templatePath[ FILENAME_MAX ];	// Template name


    // Try and get the next directory.
    //
    search = &(searchDirs[0]);
    if( NULL != search )
	nextDir = strchr( search, ':' );

    // Keep looping while there is a directory to search in, and the template
    // has not been found.
    //
    while( (NULL != search) && (NULL == result) )
    {
	// If we have multiple, then teminate this one and go to the next.
	//
	if( NULL != nextDir )
	    *(nextDir++) = '\0';

	// Create the template name.
	//
	strncpy( templatePath, search, FILENAME_MAX );
	strncat( templatePath, "/", FILENAME_MAX-1 );
	strncat( templatePath, templateName, FILENAME_MAX-1 );


	// See if the template exists, and is a regular file.
	//
	if( (0 == stat( templatePath, &statBuffer ))
	 && S_ISREG( statBuffer.st_mode ) )
	{
		result = &(templatePath[0]);
	}

	// Search on the next template directory.
	//
	else if( NULL != (search = nextDir) )
	    nextDir = index( search, ':' );
    }

    return( result );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static char*
getSearchDirectories(
    void
){
    static char  searchDirs[ 2048 ];
    *searchDirs = '\0';
    
    strncat( searchDirs, ".", sizeof(searchDirs)-1 );

    if( NULL != getenv( "TEMPLATE_DIR" ) ) {
	strncat( searchDirs, ":", sizeof(searchDirs)-1 );
        strncpy( searchDirs, getenv( "TEMPLATE_DIR" ), sizeof(searchDirs)-1 );
    }

    strncat( searchDirs, ":.templates.d:", sizeof(searchDirs)-1 );
    //
    strncat( searchDirs, getenv( "HOME" ), sizeof(searchDirs)-1 );
    strncat( searchDirs, "/.shell/templates.d:", sizeof(searchDirs)-1 );
    //
    strncat( searchDirs, "/etc/templates", sizeof(searchDirs)-1 );
    //

    return( searchDirs );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
generateFile(
    char*  templateFilename,
    char*  targetFilename
){
    // Variables.
    //
    FILE*  templateFile = NULL;
    FILE*  targetFile = NULL;


    // Make sure there are filenames....
    //
    if( (NULL != templateFilename)
     && (NULL != targetFilename) )
    {
	makeSubstitutes( templateFilename, targetFilename );

	// Announce what we're doing?
	//
	if( 0 < global::verboseLevel )
	{
	    if( 2 < global::verboseLevel )
		printf( "%s --> %s...", templateFilename, targetFilename );
	    else
		printf( "Creating: %s...", targetFilename );
	}

	// Try and open the template file.
	//
	if( NULL == (templateFile = fopen( templateFilename, "r" )) )
	{
	    if( 0 < global::verboseLevel )
	    {
	        printf( ".  [ERROR]\n    %s: %s\n",
				    templateFilename, strerror( errno ) );
	    }
	}

	// Now open the target file.  This method will report errors.
	//
	else
	    targetFile = openTargetFile( targetFilename );


	// Do the nasty.
	//
	if( (NULL != templateFile) && (NULL != targetFile) )
	{
	    createTargetFile( templateFile, targetFile );
	}
    }

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
createTargetFile(
    FILE* templateFile,
    FILE* targetFile
){
    char    buffer[ BUFSIZ ];
    size_t  bytes;
    char*   bTag;		// Begin tag.
    char*   eTag;		// End tag.
    char*   print;		// Begin printing.
    char*   subTag;		// Substitute for the tag.

    // Read in one line at a time.
    //
    while( NULL != fgets( buffer, BUFSIZ, templateFile ) )
    {
        // See if there are any tag delimiters ('%%').
	//
	if( NULL == (bTag = strstr( buffer, "%%" )) )
	{
	    fputs( buffer, targetFile );
	}

	// There may be more than one per line.
	//
	else
	{
	    // Seed the start of printing....
	    //
	    print = &(buffer[0]);

	    do
	    {
	        // Print up to the end tag.
		//
		*bTag = '\0';
		fputs( print, targetFile );

		// If there is only one, it's an error and just dump the string.
		//
		bTag += 2;
		if( NULL == (eTag = strstr( bTag, "%%" )) ) {
		    fputc( '%', targetFile );
		    fputs( bTag, targetFile );
		    bTag = NULL;
		    eTag = NULL;
		}

		// Print the substitute.
		//
		else
		{
		    *eTag++ = '\0';
		    eTag += 2;
		    fputs( getSubstitute( bTag ), targetFile );
		    print = eTag;
		}

		if( NULL != eTag )
		    bTag = strchr( eTag, '%' );
	    }
	    while( NULL != bTag );

	    // Finish off the line.
	    //
	    if( NULL != eTag )
		fputs( print, targetFile );
	}
    }

    // Announce that we're done?
    //
    if( 0 < global::verboseLevel )
	printf( ".  [done]\n" );

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static FILE*
openTargetFile(
    char* targetFilename
){
    FILE*  targetFile = NULL;
    struct stat  statBuffer;
    char   inputBuffer[ 20 ];
    bool   tryToOpen = true;
    bool   needInput = true;
    bool   beVerbose = false;
    char   newFilename[ FILENAME_MAX ];


    // File already exists.
    //
    if( 0 == stat( targetFilename, &statBuffer ) )
    {
	// If we are "quiet", just abort.
	//
	if( 0 == global::verboseLevel )
	    strncpy( inputBuffer, "abort", sizeof(inputBuffer) );
	else
	{
	    if( 2 < global::verboseLevel )
		printf( ".  [Warning]\nFile aleady exists." );
	    else
		printf( ".  [Warning]  File aleady exists.\n" );
	}

	while( needInput )
	{
	    printf( "  Abort, overwrite, or move [Aom] " );
	    fflush( stdout );

	    // Either on an error, or no input, assume abort.
	    //
	    if( NULL == fgets( inputBuffer, sizeof(inputBuffer), stdin ) )
		strncpy( inputBuffer, "abort", sizeof(inputBuffer) );

	    switch( tolower( *inputBuffer ) )
	    {
		case  'a':			// Abort
		case  '\n':
		    needInput = false;
		    tryToOpen = false;
		    beVerbose = false;
		    break;

		case  'o':			// Overwrite
		    needInput = false;
		    tryToOpen = true;
		    beVerbose = true;
		    break;

		case  'm':			// Move
		    needInput = false;
		    tryToOpen = false;
		    beVerbose = true;

		    // Move current file out of the way.
		    //
		    strncpy( newFilename, targetFilename, FILENAME_MAX );
		    strncat( newFilename, "~", FILENAME_MAX-1 );
		    (void)unlink( newFilename );
		    if( (0 == link( targetFilename, newFilename ))
		     && (0 == unlink( targetFilename )) )
		    {
			tryToOpen = true;
		    }
		    break;
	    }
	}

    }

    // Try and open?
    //
    if( tryToOpen )
    {
        if( beVerbose && (0 < global::verboseLevel) )
	{
	    printf( "      + creating: %s...", targetFilename );
	    fflush( stdout );
	}


        if( NULL == (targetFile = fopen( targetFilename, "w" )) )
	{
	    if( 0 < global::verboseLevel )
		printf( ".  [ERROR]\n    Couldn't open \"%s\": %s",
		    targetFilename, strerror( errno ) );
	}
    }


    return( targetFile );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static char*
getSubstitute(
    char*  tag
){
    char*  result = NULL;
    static char   buffer[ 48 ];
    int    isEqual;

    for( int sub=0; (NULL != substitute[sub].key) && (NULL == result); ++sub )
    {
        if( substitute[sub].ign )
	    isEqual = strcasecmp( tag, substitute[sub].key );
	else
	    isEqual = strcmp( tag, substitute[sub].key );

	if( 0 == isEqual )
	    result = substitute[sub].val;
    }

    if( NULL == result )
    {
        snprintf( buffer, sizeof(buffer), "%%%s%%", tag );
	result = buffer;
    }

    return( result );
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
makeSubstitutes(
    const char*  templateFilename,
    const char*  targetFilename
){
    int    sub = 0;
    char*  begin;
    char*  extension;
    char   buffer[ FILENAME_MAX ];
    char   fileHeader[ 82 ];


    // Target name
    //
    if( NULL != targetFilename )
    {
	strncpy( buffer, targetFilename, FILENAME_MAX );

	if( NULL != (begin = strrchr( buffer, '/' )) )
	    ++begin;
	else
	    begin = buffer;
	extension = strrchr( begin, '.' );

	substitute[sub].key = strdup( "fIlename" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;


	// File header.
	//
        snprintf( fileHeader, sizeof(fileHeader), "//~~ %s %s%s",
		begin,
		"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
		"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~" );
	fileHeader[80] = '\0';
	substitute[sub].key = strdup( "fileheader" );
	substitute[sub].val = strdup( fileHeader );
	substitute[sub].ign = true;
	sub++;

        snprintf( fileHeader, sizeof(fileHeader), "/*== %s %s%s",
		begin,
		"==========================================================",
		"==========================================================" );
	fileHeader[78] = '*';
	fileHeader[79] = '\\';
	fileHeader[80] = '\0';
	substitute[sub].key = strdup( "header-dashes" );
	substitute[sub].val = strdup( fileHeader );
	substitute[sub].ign = true;
	sub++;


	if( NULL != extension )
	{
	    substitute[sub].key = strdup( "eXtension" );
	    substitute[sub].val = strdup( extension+1 );
	    substitute[sub].ign = false;
	    sub++;

	    *extension = '\0';
	}

	substitute[sub].key = strdup( "bAsename" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	if( NULL != extension )
	    *extension = '.';



	// Lower case
	//
	for( char* psz=begin; '\0' != *psz; ++psz )
	    *psz = tolower( *psz );

	substitute[sub].key = strdup( "filename" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	if( NULL != extension )
	{
	    substitute[sub].key = strdup( "extension" );
	    substitute[sub].val = strdup( extension+1 );
	    substitute[sub].ign = false;
	    sub++;

	    *extension = '\0';
	}

	substitute[sub].key = strdup( "basename" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	*begin = toupper( *begin );
	substitute[sub].key = strdup( "Basename" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	if( NULL != extension )
	    *extension = '.';


	// Upper case
	//
	for( char* psz=begin; '\0' != *psz; ++psz )
	    *psz = toupper( *psz );

	substitute[sub].key = strdup( "FILENAME" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	if( NULL != extension )
	{
	    substitute[sub].key = strdup( "EXTENSION" );
	    substitute[sub].val = strdup( extension+1 );
	    substitute[sub].ign = false;
	    sub++;

	    *extension = '\0';
	}

	substitute[sub].key = strdup( "BASENAME" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = false;
	sub++;

	if( NULL != extension )
	    *extension = '.';
    }


    // Template name
    //
    if( NULL != templateFilename )
    {
    //     char* strrchr(const char *s, int c);
    //
//	if( NULL == (begin = strrchr( templateFilename, '/' )) )
	if( NULL == (begin = (char*)strrchr(templateFilename, '/' )) )
	    begin = (char*)templateFilename;
	else
	    begin++;

	substitute[sub].key = strdup( "template" );
	substitute[sub].val = strdup( begin );
	substitute[sub].ign = true;
	sub++;
	//
	substitute[sub].key = strdup( "templatepath" );
	substitute[sub].val = strdup( templateFilename );
	substitute[sub].ign = true;
	sub++;
    }


    // Dates
    //
    long t1 = time(0);
    const struct tm*  t = localtime( &t1 );
    const struct tm*  utc = gmtime( &t1 );

    substitute[sub].key = strdup( "shortdate" );
    strftime( buffer, FILENAME_MAX, "%m/%d/%y", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "date" );
    strftime( buffer, FILENAME_MAX, "%b %d, %Y", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "longdate" );
    strftime( buffer, FILENAME_MAX, "%A %B %d, %Y", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "yymmdd" );
    strftime( buffer, FILENAME_MAX, "%y%m%d", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "ccyymmdd" );
    strftime( buffer, FILENAME_MAX, "%Y%m%d", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "yy-mm-dd" );
    strftime( buffer, FILENAME_MAX, "%y-%m-%d", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "ccyy-mm-dd" );
    strftime( buffer, FILENAME_MAX, "%Y-%m-%d", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "iso8601date" );
    strftime( buffer, FILENAME_MAX, "%Y%m%dT%H%M%S", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "utc8601date" );
    strftime( buffer, FILENAME_MAX, "%Y%m%dT%H%M%Sz", utc );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "time" );
    strftime( buffer, FILENAME_MAX, "%X", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "longtime" );
    strftime( buffer, FILENAME_MAX, "%H:%M:%S %z", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "datetime" );
    strftime( buffer, FILENAME_MAX, "%Y-%m-%d %H:%M:%S %Z", t );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "utcdatetime" );
    strftime( buffer, FILENAME_MAX, "%Y-%m-%d %H:%M:%S %Z", utc );
    substitute[sub].val = strdup( buffer );
    substitute[sub].ign = true;
    sub++;


    // Names and Email (hard-coded for now)
    substitute[sub].key = strdup( "author" );
    substitute[sub].val = strdup( "Andy Pippin" );
    substitute[sub].ign = true;
    sub++;

    substitute[sub].key = strdup( "email" );
    substitute[sub].val = strdup( "andy@ajpippin.us" );
    substitute[sub].ign = true;
    sub++;


    // End of list.
    //
    substitute[ sub ].key = NULL;
    substitute[ sub ].val = NULL;
    substitute[ sub ].ign = false;

    return;
}

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static char*
fixNull(
    char*  string,
    char*  delim
){
    static char  fixed[ 1024 ];

    // If the string is NULL, say so.
    //
    if( NULL == string )
    {
        strncpy( fixed, "NULL", sizeof(fixed) );
    }

    // If the string is not NULL, but the delimiter is, don't decorate.
    //
    else if( NULL == delim )
    {
        strncpy( fixed, string, sizeof(fixed) );
    }

    // Decorate with the delimiter.
    //
    else
    {
        strncpy( fixed, delim, sizeof(fixed) );
        strncat( fixed, string, sizeof(fixed)-1 );
        strncat( fixed, delim, sizeof(fixed)-1 );
    }
    

    return( (char*)fixed );
}


//==============================================================================
