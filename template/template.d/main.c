%%header-dashes%%
|*                                                                            *|
|*  Entry point into %%bAsename%%.					      *|
|*                                                                            *|
|*                                                                            *|
|*        Copyright (c) %%year%% %%author%% <%%email%%>                  *|
|*                                                                            *|
|*    Permission is hereby granted, free of charge, to any person obtaining   *|
|*  a copy of this software and associated documentation files (the           *|
|*  "Software"), to deal in the Software without restriction, including       *|
|*  without limitation the rights to use, copy, modify, merge, publish,       *|
|*  distribute, sublicense, and/or sell copies of the Software, and to        *|
|*  permit persons to whom the Software is furnished to do so, subject to     *|
|*  the following conditions:                                                 *|
|*                                                                            *|
|*    The above copyright notice and this permission notice shall be          *|
|*  included in all copies or substantial portions of the Software.           *|
|*                                                                            *|
|*    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,         *|
|*  EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF        *|
|*  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.    *|
|*  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY      *|
|*  CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,      *|
|*  TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE         *|
|*  SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.                    *|
|*                                                                            *|
|*                                                                            *|
\*============================================================================*/

%%header-dashes%%
|*									      *|
|*  Entry point into %%bAsename%%.					      *|
|*									      *|
|* @author   %%author%% <%%email%%>				      *|
|*									      *|
\*============================================================================*/


//=[ Identity ]=================================================================

#define __%%BASENAME%%_%%EXTENSION%%__


//=[ Includes ]=================================================================

#include  <getopt.h>
#include  <libgen.h>
#include  <stdio.h>
#include  <stdlib.h>
#include  <string.h>
#include  <unistd.h>


//=[ Typedefs/Structures/Enums ]================================================

//=[ Constants ]================================================================

#define  DEFAULT_Verbosity	1
#define  PROGRAM_VERSION	"v0.9"


//=[ Variables ]================================================================

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


// Global verbose level.
//
int verboseLevel = DEFAULT_Verbosity;

static char*  g_cmdName = "%%bAsename%%";


//=[ Prototypes ]===============================================================

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 * Invokes user-defined code (duh!).
 *
 * @param numArgs  Number of arguments on the command line.
 * @param  args  Arguments from the command line.
 * @param  envs  Environmental variables.
 * @return  Exit code.
 */
	int 
main(
    int		numArgs,
    char*	args[],
    char*	envs[]
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 * Display correct usage (synopsis) of the application.
 *
 * @param  programName  Application name.
 * @param  prefix  What to put before each line.
 * @param  outStream  Stream to display output.
 */
	static void
printUsage(
    char*	programName,
    char*	prefix,
    FILE*	outStream
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Display complete description of the options.
 *
 * @param  helpTopic  Topic of interest.
 * @param  programName  Application name.
 * @param  prefix  What to put before each line.
 * @param  outStream  Stream to display output.
 */
	static void
printHelp(
    char*	programName,
    char*	helpTopic,
    char*	prefix,
    FILE*	outStream
);


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 * Writes the application name and version to stdout.
 *
 * @param  programName - Application name.
 * @param  prefix -      What to put before each line.
 * @param  outStream -   Stream to display output.
 */
	static void
printVersion(
    char*	programName,
    char*	prefix,
    FILE*	outStream
);



//=[ Implementation ]===========================================================

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


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
printVersion(
    char*	programName,
    char*	prefix,
    FILE*	outStream
){
    // Fix parameters.
    //
    programName = (NULL != programName) ? programName : g_cmdName;
    prefix = (NULL != prefix) ? prefix : (char*)"";
    outStream = (NULL != outStream) ? outStream : stdout;

    fprintf( outStream, "%s%s %s\n", prefix, programName, PROGRAM_VERSION );
    fflush( outStream );

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void 
printUsage(
    char*	programName,
    char*	prefix,
    FILE*	outStream
){
    // Fix parameters.
    //
    programName = (NULL != programName) ? programName : g_cmdName;
    prefix = (NULL != prefix) ? prefix : (char*)"";
    outStream = (NULL != outStream) ? outStream : stdout;

    fprintf( outStream, "%sUsage: %s [-hqVv]\n", prefix, programName );
    fflush( outStream );

    return;
}


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

	static void
printHelp(
    char*	programName,	
    char*	helpTopic,
    char*	prefix,
    FILE*	outStream
){
    // Local variables.
    //
    char  isNull;
    char  isAll;

    // Set flags.
    //
    isNull = (NULL == helpTopic) ? 1 : 0;
    if( ! isNull )
	isAll = (0 == strcasecmp( "all", helpTopic )) ? 1 : 0;


    // Fix parameters.
    //
    programName = (NULL != programName) ? programName : g_cmdName;
    helpTopic = (NULL != helpTopic) ? helpTopic : (char*)"";
    prefix = (NULL != prefix) ? prefix : (char*)"";
    outStream = (NULL != outStream) ? outStream : stdout;


    // *** Now start displaying help. ***

    // Description
    //
    if( isNull || isAll || IsEqual( helpTopic, "desc" ) )
    {
        fprintf( outStream,
		"%s\"%s\" ...\n",
	    prefix,
	    programName );
    }

    // Usage
    //
    if( isNull || isAll || IsEqual( helpTopic, "usage" ) )
    {
	printUsage( programName, prefix, outStream );
    }

    // Options
    //
    if( isNull || isAll || IsEqual( helpTopic, "option" ) )
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
    if( isAll || IsEqual( helpTopic, "topic" ) )
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


//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

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


    // Initialize.
    //
    g_cmdName = (char*)basename( args[ 0 ] );


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
	    printHelp( g_cmdName, optarg, "", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** USAGE ***
          //
          case  1:
	    printUsage( g_cmdName, "", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** VERBOSE ***
          //
          case  'v':
            if( NULL == optarg )
                verboseLevel++;
            else
                verboseLevel = atoi( optarg );
            break;


	  // *** VERSION ***
	  //
	  case  'V':
	    printVersion( g_cmdName, "", stdout );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** QUIET ***
          //
          case  'q':
	    verboseLevel = 0;
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
	printUsage( g_cmdName, "", stderr );
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
