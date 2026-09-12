%%fileheader%%
/*
 *  Entry point into %%bAsename%%.
 *
 * @author:    %%author%% <%%email%%>
 * @requires:   
 */

//-[ Identity ]-----------------------------------------------------------------

#define __%%BASENAME%%_%%EXTENSION%%__


//-[ Includes ]-----------------------------------------------------------------

#include  <getopt.h>
#include  <libgen.h>
#include  <stdio.h>
#include  <stdlib.h>
#include  <string.h>
#include  <unistd.h>


//-[ Typedefs/Structures/Enums ]------------------------------------------------

//-[ Constants ]----------------------------------------------------------------

int const  DEFAULT_Verbosity = 1;

const char*  VERSION_date = (const char*)"<:r ! date>";
const char*  VERSION_number = (const char*)"<version number>";


//-[ Variables ]----------------------------------------------------------------

// Command line options.
//
struct option  optionList[] = 
{
    { "help",		optional_argument,	NULL,	'h' },
    { "quiet",		no_argument,		NULL,	'q' },
    { "usage",		no_argument,		NULL,	 1  },
    { "version",	no_argument,		NULL,	'V' },
    { "verbose",	optional_argument,	NULL,	'v' },

    { NULL,		no_argument,		NULL,	 0 }
};
#define  SHORT_OPTIONS  "hqvV"



//
namespace global
{
    /**  Verbose level. */
    int verboseLevel = DEFAULT_Verbosity;

    /**  Invocation name. */
    const char* cmdName = (const char*)"%%bAsename%%";
};


//-[ Prototypes ]---------------------------------------------------------------

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
// @NAME:	main
//
// @RESULT:	Invokes user-defined code (duh!).
//
// @PARAMS:	numArgs - Number of arguments on the command line.
//		args -    Arguments from the command line.
//		envs -    Environmental variables.
//
// @RETURNS:	Exit code.
//
	int 
main(
    int		numArgs,
    char*	args[],
    char*	envs[]
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
// @NAME:	printUsage
//
// @RESULT:	Display correct usage (synopsis) of the application.
//
// @PARAMS:	programName - Application name (NULL --> default).
//		prefix -      What to put before each line (NULL --> nothing).
//		outStream -   Stream to display output (NULL --> stdout).
//
// @RETURNS:	Nothing.
//
	static void
printUsage(
    const char*	programName,
    const char*	prefix,
    FILE*	outStream =NULL
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
// @NAME:	printHelp
//
// @RESULT:	Display complete description of the options.
//
// @PARAMS:	programName - Application name (NULL --> default).
//		helpTopic -   Topic of interest (NULL --> all).
//		prefix -      What to put before each line (NULL --> nothing).
//		outStream -   Stream to display output (NULL --> stdout).
//
// @RETURNS:	Nothing.
//
	static void
printHelp(
    const char*	programName =NULL,
    char*	helpTopic =NULL,
    char*	prefix =NULL,
    FILE*	outStream =NULL
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
// @NAME:	printVersion
//
// @RESULT:	Writes the application name and version to stdout.
//
// @PARAMS:	programName - Application name (NULL --> default).
//		prefix -      What to put before each line (NULL --> nothing).
//		outStream -   Stream to display output (NULL --> stdout).
//
// @RETURNS:	Nothing.
//
	static void
printVersion(
    const char*	programName,
    const char*	prefix,
    FILE*	outStream =NULL
);



//-[ Implementation ]-----------------------------------------------------------

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
	static char
IsEqual(
    char*	str1,
    char*	str2
){
    char  result = 0;
    int   len1, len2;

    if( (NULL != str1) && (NULL != str2) )
    {
        len1 = strlen( str1 );
        len2 = strlen( str2 );

        result = !strncasecmp( str1, str2, ((len1 < len2) ? len1 : len2) );
    }

    return( result );
}


//~ printVersion ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
	static void
printVersion(
    const char*	_programName,
    const char*	_prefix,
    FILE*	_outStream
){
    // Fix parameters.
    //
    char* programName = (NULL != _programName) ? (char*)_programName : (char*)global::cmdName;
    char* prefix = (NULL != _prefix) ? (char*)_prefix : (char*)"";
    FILE* outStream = (NULL != _outStream) ? _outStream : stdout;

    fprintf( outStream, "%s%s %s v%s\n", prefix, programName, VERSION_date, VERSION_number );
    fflush( outStream );

    return;
}


//~ printUsage ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
	static void 
printUsage(
    const char*	_programName,
    const char*	_prefix,
    FILE*	_outStream
){
    // Fix parameters.
    //
    char* programName = (NULL != _programName) ? (char*)_programName : (char*)global::cmdName;
    char* prefix = (NULL != _prefix) ? (char*)_prefix : (char*)"";
    FILE* outStream = (NULL != _outStream) ? _outStream : stdout;

    fprintf( outStream, "%sUsage: %s [-hqVv]\n", prefix, programName );
    fflush( outStream );

    return;
}


//~ printHelp ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
//
	static void
printHelp(
    const char*	_programName,	
    char*	helpTopic,
    char*	prefix,
    FILE*	outStream
){
    // Local variables.
    //
    bool  isNull;
    bool  isAll;

    // Set flags.
    //
    isNull = (NULL == helpTopic) ? true : false;
    if( ! isNull )
	isAll = (0 == strcasecmp( "all", helpTopic )) ? true : false;


    // Fix parameters.
    //
    char* programName = (NULL != _programName) ? (char*)_programName : (char*)global::cmdName;
    helpTopic = (NULL != helpTopic) ? helpTopic : (char*)"";
    prefix = (NULL != prefix) ? prefix : (char*)"";
    outStream = (NULL != outStream) ? outStream : stdout;


    // *** Now start displaying help. ***

    // Description
    //
    if( isNull || isAll || IsEqual( helpTopic, (char*)"desc" ) )
    {
        fprintf( outStream,
		"%s\"%s\" ...\n",
	    prefix,
	    programName );
    }

    // Usage
    //
    if( isNull || isAll || IsEqual( helpTopic, (char*)"usage" ) )
    {
	printUsage( programName, prefix, outStream );
    }

    // Options
    //
    if( isNull || isAll || IsEqual( helpTopic, (char*)"option" ) )
    {
        fprintf( outStream,
"%sIf a long option shows an argument as mandatory, then it is mandatory\n\
%sfor the equivalent short option also.  Similarly for optional arguments.\n",
	    prefix,
	    prefix );

	// Main operation.
	//
        fprintf( outStream, "%s** Main Operation Modes **\n", prefix );


	// Informative/Debug options.
	//
        fprintf( outStream, "%s** Informative Output **\n", prefix );
        fprintf( outStream, 
"%s-h, --help[=topic]     Display this help message or details on a topic.\n\
%s    --usage            Display synopsis of command line arguments.\n\
%s-v, --verbose[=level]  Set verbosity (def = %d)\n\
%s-q, --quiet            Set verbosity to 0.\n\
%s-V, --version          Print version to stdout and exit.\n",
		prefix,
		prefix,
		prefix, DEFAULT_Verbosity,
		prefix,
		prefix );
    }

    // Topics
    //
    if( isAll || IsEqual( helpTopic, (char*)"topic" ) )
    {
        fprintf( outStream, 
"%s  all     Display this help message or details on a topic.\n\
%s  desc    Description of this command.\n\
%s  topic   Help topics.\n\
%s  usage   Synopsis of command line arguments.\n\
%s  option  Description of command line arguments.\n",
		prefix,
		prefix,
		prefix,
		prefix,
		prefix );
    }

    fflush( outStream );
    return;
}


//~ main ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	int
main(
    int		numArgs,
    char*	args[],
    char*	envs[]
){
    // Local variables.
    //
    int   exitCode = 0;				// Exit code.
    int   shortOption = 0;			// Short option value.
    int   longOption = 0;			// Long option value.
    char  cmdError = 0;				// Command line parsing error.
    char* cmdName;				// Command name.


    // Initialize.
    //
    global::cmdName = (char*)basename( args[ 0 ] );


    // Parse the command line arguments.
    //
    while( EOF != (shortOption = getopt_long( numArgs, args, SHORT_OPTIONS,
			optionList, &longOption )) )
    {
	switch( shortOption )
	{
	  // *** HELP ***
	  //
	  case  'h':
	    printHelp( cmdName, optarg, (char*)"", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** USAGE ***
          //
          case  1:
	    printUsage( cmdName, (char*)"", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** VERBOSE ***
          //
          case  'v':
            if( NULL == optarg )
                global::verboseLevel++;
            else
                global::verboseLevel = atoi( optarg );
            break;


	  // *** VERSION ***
	  //
	  case  'V':
	    printVersion( cmdName, (char*)"", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** QUIET ***
          //
          case  'q':
	    global::verboseLevel = 0;
            break;


	  // *** ERROR ***
	  //
	  case  '?':
	  default:
	    cmdError = 1;
	    break;
	}
    }


    // Make sure we have the right number of items to work with.
    //
    if( 0 == (numArgs - optind) )
    {
        cmdError = 1;
    }


    // Print error message.
    //
    if( cmdError )
    {
	printUsage( cmdName, "", stderr );
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
	}
    }


    // Exit.
    //
    return( exitCode );
}


//= End of file ================================================================
