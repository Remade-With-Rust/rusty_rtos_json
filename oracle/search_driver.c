/* The C arm of K7's coreJSON QUERY differential: JSON_SearchConst and
 * JSON_Iterate, call for call.
 *
 * `core_json.c` is compiled VERBATIM out of the pinned checkout; nothing here
 * copies or edits it. This driver only decides WHAT to ask, and prints every
 * observable of each answer so a transcription that got the status right and
 * the offset wrong fails on the offset.
 *
 * Three workloads, and they exist for different reasons:
 *
 *   1. A table of documents crossed with a table of queries. The documents are
 *      chosen for the places a query engine drifts -- a key containing the
 *      separator, a key containing a bracket, an empty key, an empty string
 *      value, duplicate keys, deep nesting. The queries are chosen for the
 *      places the QUERY PARSER drifts -- an empty part, a trailing separator,
 *      a doubled separator, an unterminated bracket, an index at and past
 *      UINT32_MAX.
 *
 *   2. Both APIs over all 318 JSONTestSuite files. 188 of those are malformed
 *      on purpose, and the search path does NOT validate first -- it walks
 *      whatever it is handed. That is exactly where a reimplementation runs
 *      off the end, so the adversarial corpus earns its keep twice.
 *
 *   3. JSON_Iterate driven to exhaustion over every document, so the final
 *      status is compared and not just the pairs.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "core_json.h"

/* Documents. Valid JSON except where the corpus covers that separately; the
 * interesting ones are the keys that collide with the QUERY grammar. */
static const char *const DOCS[] = {
    "{\"a\":1}",
    "{\"a\":{\"b\":{\"c\":42}}}",
    "{\"a\":[1,2,3]}",
    "[1,2,3]",
    "[[1,2],[3,4]]",
    "{\"a\":[{\"b\":1},{\"b\":2}]}",
    "{\"x\":\"hello\"}",
    "{\"x\":\"\"}",
    "{\"x\":\"a\\\"b\"}",
    "{\"x\":\"\\u00e9\"}",
    "{\"a.b\":1}",
    "{\"a[0]\":1}",
    "{\"\":1}",
    "{\"a\":true,\"b\":false,\"c\":null}",
    "{\"a\":-1.5e10}",
    "{ \"a\" : 1 , \"b\" : 2 }",
    "{\"a\":1,\"a\":2}",
    "[]",
    "{}",
    "{\"a\":[]}",
    "{\"a\":{}}",
    "[{\"a\":1},{\"b\":2}]",
    "{\"a\":[[[\"deep\"]]]}",
    "{\"big\":[0,1,2,3,4,5,6,7,8,9]}",
    "",
    "   ",
    "\"bare\"",
    "42",
    /* Long enough to drive the iteration loop past the 32-pair cap. */
    "[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39]",
    "{\"k0\":0,\"k1\":1,\"k2\":2,\"k3\":3,\"k4\":4,\"k5\":5,\"k6\":6,\"k7\":7,\"k8\":8,\"k9\":9,\"k10\":10,\"k11\":11}",
};

/* Queries. Half of these are meant to be refused. */
static const char *const QUERIES[] = {
    "a", "b", "x", "c", "big",
    "a.b", "a.b.c", "a.b.c.d",
    "", ".", "..", "a.", ".a", "a..b",
    "[0]", "[1]", "[2]", "[3]", "[10]",
    "[4294967295]", "[4294967296]", "[99999999999999999999]",
    "a[0]", "a[1]", "a[3]", "a[0].b", "a.b[0]",
    "[0][0]", "[0][1]", "[1][0]",
    "a[0][0][0]", "big[9]", "big[10]",
    "a.b]", "a[", "a[]", "a[-1]", "a.b.", "a[0]b",
};

#define N_DOCS    ( sizeof( DOCS ) / sizeof( DOCS[ 0 ] ) )
#define N_QUERIES ( sizeof( QUERIES ) / sizeof( QUERIES[ 0 ] ) )

/* Cap on pairs reported per document: the corpus holds a 100,000-bracket
 * file, and an unbounded loop would make the trace about that one file. */
#define MAX_PAIRS 32

static const char *status_name( JSONStatus_t s )
{
    switch( s )
    {
        case JSONPartial:         return "Partial";
        case JSONSuccess:         return "Success";
        case JSONIllegalDocument: return "IllegalDocument";
        case JSONMaxDepthExceeded:return "MaxDepthExceeded";
        case JSONNotFound:        return "NotFound";
        case JSONNullParameter:   return "NullParameter";
        case JSONBadParameter:    return "BadParameter";
        default:                  return "UNKNOWN";
    }
}

static const char *type_name( JSONTypes_t t )
{
    switch( t )
    {
        case JSONInvalid: return "Invalid";
        case JSONString:  return "String";
        case JSONNumber:  return "Number";
        case JSONTrue:    return "True";
        case JSONFalse:   return "False";
        case JSONNull:    return "Null";
        case JSONObject:  return "Object";
        case JSONArray:   return "Array";
        default:          return "UNKNOWN";
    }
}

/* One search, printed with every observable it produces. The value is
 * reported as an OFFSET into the buffer, not a pointer: a pointer is a
 * machine fact and an offset is a fact about the document. */
static void one_search( const char *tag,
                        const char *buf,
                        size_t len,
                        const char *query )
{
    const char *value = NULL;
    size_t valueLength = 0U;
    JSONTypes_t type = JSONInvalid;
    JSONStatus_t r;

    r = JSON_SearchConst( buf, len, query, strlen( query ),
                          &value, &valueLength, &type );

    if( r == JSONSuccess )
    {
        printf( "%s %s %zu %zu %s\n", tag, status_name( r ),
                ( size_t ) ( value - buf ), valueLength, type_name( type ) );
    }
    else
    {
        /* On a refusal the outputs are untouched, so printing them would be
         * printing our own initialisers on both arms and proving nothing. */
        printf( "%s %s\n", tag, status_name( r ) );
    }
}

static void all_pairs( const char *tag, const char *buf, size_t len )
{
    size_t start = 0U, next = 0U;
    JSONPair_t pair;
    JSONStatus_t r;
    int n = 0;

    for( ;; )
    {
        memset( &pair, 0, sizeof pair );
        r = JSON_Iterate( buf, len, &start, &next, &pair );

        if( r != JSONSuccess )
        {
            printf( "%s end %s\n", tag, status_name( r ) );
            break;
        }

        printf( "%s pair %d ", tag, n );

        if( pair.key == NULL )
        {
            printf( "nokey " );
        }
        else
        {
            printf( "key %zu %zu ", ( size_t ) ( pair.key - buf ), pair.keyLength );
        }

        printf( "%zu %zu %s\n", ( size_t ) ( pair.value - buf ),
                pair.valueLength, type_name( pair.jsonType ) );

        n++;

        if( n >= MAX_PAIRS )
        {
            printf( "%s capped\n", tag );
            break;
        }
    }
}

int main( int argc, char **argv )
{
    size_t d, q;
    /* Long enough for the longest corpus filename plus a query. At 64 this
     * silently truncated the name, so several files shared one tag and the
     * differential would have been comparing lines that were not the same
     * question. The odd short names in the trace are what gave it away. */
    char tag[ 1024 ];

    if( argc < 2 )
    {
        fprintf( stderr, "usage: search_driver <corpus-dir>\n" );
        return 2;
    }

    printf( "geometry docs=%zu queries=%zu cap=%d\n",
            N_DOCS, N_QUERIES, MAX_PAIRS );

    /* 1. the document x query cross product */
    for( d = 0; d < N_DOCS; d++ )
    {
        size_t len = strlen( DOCS[ d ] );

        printf( "doc %zu %zu\n", d, len );

        for( q = 0; q < N_QUERIES; q++ )
        {
            ( void ) snprintf( tag, sizeof tag, "search %zu %zu", d, q );
            one_search( tag, DOCS[ d ], len, QUERIES[ q ] );
        }

        ( void ) snprintf( tag, sizeof tag, "iter %zu", d );
        all_pairs( tag, DOCS[ d ], len );
    }

    /* 2 and 3. both APIs over the whole corpus, malformed files included */
    {
        FILE *list = fopen( argv[ 1 ], "r" );
        char name[ 512 ];
        static char filebuf[ 1 << 20 ];

        if( list == NULL )
        {
            fprintf( stderr, "cannot open the file list %s\n", argv[ 1 ] );
            return 2;
        }

        while( fgets( name, sizeof name, list ) != NULL )
        {
            char path[ 1024 ];
            size_t n;
            FILE *f;
            size_t k = strlen( name );

            while( ( k > 0U ) && ( ( name[ k - 1U ] == '\n' ) || ( name[ k - 1U ] == '\r' ) ) )
            {
                name[ --k ] = '\0';
            }

            if( k == 0U )
            {
                continue;
            }

            ( void ) snprintf( path, sizeof path, "test_parsing/%s", name );
            f = fopen( path, "rb" );

            if( f == NULL )
            {
                fprintf( stderr, "cannot open %s\n", path );
                return 2;
            }

            n = fread( filebuf, 1, sizeof filebuf, f );
            fclose( f );

            printf( "file %s %zu\n", name, n );

            ( void ) snprintf( tag, sizeof tag, "fsearch %s a", name );
            one_search( tag, filebuf, n, "a" );
            ( void ) snprintf( tag, sizeof tag, "fsearch %s [0]", name );
            one_search( tag, filebuf, n, "[0]" );
            ( void ) snprintf( tag, sizeof tag, "fsearch %s a.b", name );
            one_search( tag, filebuf, n, "a.b" );

            ( void ) snprintf( tag, sizeof tag, "fiter %s", name );
            all_pairs( tag, filebuf, n );
        }

        fclose( list );
    }

    printf( "end\n" );
    return 0;
}
