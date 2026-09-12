%%fileheader%%
/*									      *\
|*  Entry point into %%bAsename%%.					      *|
|*									      *|
|* @author   %%author%%  <%%email%%>				      *|
|* @require								      *|
|*									      *|
\*============================================================================*/

//-[ Identity ]-----------------------------------------------------------------

#define __%%BASENAME%%_%%EXTENSION%%__


//-[ Includes ]-----------------------------------------------------------------

#include  <getopt.h>
#include  <stdio.h>
#include  <stdlib.h>
#include  <string.h>


//-[ Typedefs/Structures/Enums ]------------------------------------------------

//-[ Constants ]----------------------------------------------------------------

// Command line options.
//
struct option  LONG_OPTIONS[] = 
{
    { "help",		optional_argument,	NULL,	'h' },
    { "quiet",		no_argument,		NULL,	'q' },
    { "usage",		no_argument,		NULL,	 1  },
    { "verbose",	optional_argument,	NULL,	'v' },

    { NULL,		no_argument,		NULL,	 0 }
};
const char* const  SHORT_OPTIONS = "hqv";


//-[ Variables ]----------------------------------------------------------------

namespace global
{
    /**  Verbose level. */
    int verboseLevel = 1;
};

/**  Invocation name. */
char* cmdName = "%%bAsename%%";


//-[ Prototypes ]---------------------------------------------------------------

//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/**
 *  Main entry point into program.  (Like, duh!)
 *
 * @param   numArgs  Number of arguments on the command line.
 * @param   args     Arguments from the command line.
 * @return  Exit code.
 */
	int 
main(
    int		numArgs,
    char*	args[]
);


//-[ Implementation ]-----------------------------------------------------------

	int
main(
    int    numArgs,
    char*  cmdLine[]
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
    cmdName = cmdLine[ 0 ];

    // Parse the command line arguments.
    //
    while( EOF != (shortOption = getopt_long( numArgs, cmdLine, SHORT_OPTIONS,
			LONG_OPTIONS, &longOption )) )
    {
	switch( shortOption )
	{
	  // *** HELP ***
	  //
	  case  'h':
	    printf( "Need to print some help!\n" );
            return( 0 );
	    // ***                                    ***
	    // *** DOES NOT CONTINUE -- PROGRAM EXITS ***
	    // ***                                    ***


          // *** USAGE ***
          //
          case  1:
	    printf( "Need to print some usage!\n" );
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
//        cmdError = 1;
    }


    // Print error message.
    //
    if( cmdError )
    {
	printf( "Need to print some usage!\n" );
	exitCode = 1;
    }

    // Do the work.
    //
    else
    {
	// Loop through items.
	//
	for( cmdLine = &(cmdLine[optind]); NULL != *cmdLine; ++cmdLine )
	{
	}
    }


    // Exit.
    //
    return( exitCode );
}


//= End of file ================================================================
