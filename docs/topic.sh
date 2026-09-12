#!/usr/bin/env bash

# Topics
topic_dir="${HOME}/.topics"
verbose=${TOPIC_verbose:-1}

# helper
unset find_topic_file
function find_topic_file() {
  file=

  num="`basename $1 .txt`";
  num="`basename $num .md`";
  for i in 000${num} 00${num} 0${num} ${num}; do
    if [ -f ${topic_dir}/${i}.txt ]; then
      file="${topic_dir}/${i}.txt"
      break;
    elif [ -f ${topic_dir}/${i}.md ]; then
      file="${topic_dir}/${i}.md"
      break;
    fi
  done

  echo "${file}"
}

unset create_regexp
function create_regexp() {
  prefix='('
  search=''
  suffix=''

  for term in "${@}"; do
    #echo "\${term}=\"${term}\", \${search}=\"${search}\""
    search="${search}${prefix}${term}"
    prefix="|"
    suffix=')'
  done
  
  echo "${search}${suffix}"
}


# Search keywords.
unset keyword_search
function keyword_search () {
  search=$(create_regexp "${@}")
  grep --color=never -E '(@topic|@search)' ${topic_dir}/????.{txt,md} |\
      grep -i -E "${search}" |\
      sed -e 's/.txt:@topic/:/' -e 's/.md:@topic/:/' -e 's/.txt:@search/:/' -e 's/.md:@search/:/' -e "s,^${topic_dir}/,," |\
      sort
}

# Total context search
unset full_context_search
function full_context_search () {
  search=$(create_regexp "${@}")
  grep -i --color=never -E "${search}" ${topic_dir}/????.{txt,md} |\
      sed -e 's,txt:,txt: ,' -e 's,md:,md: ,' -e "s,^${topic_dir}/,," |\
      sort
}

# Display note
unset display_topic
function display_topic ()
{
  file="`find_topic_file $1`"
  if [ "X${file}" = "X" ]; then
      echo "${1}: no topic found"
  else
    cols=$(stty -a | grep columns | tr ';' '\n' | grep columns | awk '{print $2}')
    ( echo ""
      echo "-- `basename ${file}` ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------" | cut -c 1-${cols};
      cat "${file}"
      echo "------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------" | cut -c 1-${cols};
      echo "";
    ) | less --quit-at-eof --no-init
  fi;
}


# Edit note.
unset edit_topic
function edit_topic () {
  file="$(find_topic_file ${1})"

  if [ "X${file}" != "X" ]; then
#    echo "Editing: \"${file}\""; sleep 1
    git_pull "${file}"
    ${VISUAL:-vi} "${file}"
    sed -i -e "s@Last updated: 20[0-9][0-9]-[0-9][0-9]-[0-9][0-9]@Last updated: $(date +'%Y-%m-%d')@" "${file}" 
    echo "Pushing to Git..."
    pushd ~/.topics && git commit "${file}" && git push origin && popd || exit 99
  else
      echo "${1}: no topic found"
  fi
}


unset git_pull
function git_pull() {
  touch /dev/null # just to get something in the function
##  echo 'tick'
##    file="${1}"
##    prompt="${2}"
##  
##    _pull="yes"
##  
##    read -p "Update (pull) the topic first? [Yn]: " _pull
##    if [[ "X${_pull}" = "X" ]]; then
##      _pull="yes"
##    fi
##  
##    case "${_pull}" in
##      [Nn]*)  _pull=no  ;;
##      *)      _pull=yes ;;
##    esac
##  
##    if [[ ${_pull} = "yes" ]]; then
##      pushd "${topic_dir}" >/dev/null
##      git fetch --all  >/dev/null
##      f=`echo ${file} | sed -e "s,${topic_dir}/,,"`
##      git fetch origin -- ${f} >/dev/null
##      popd >/dev/null
##    fi
##  
##    sleep 3
}

unset git_commit
function git_commit() {
  touch /dev/null # just to get something in the function
##  echo 'tock'
##    file="${1}"
##    prompt="${2}"
##  
##    _commit="yes"
##  
##    read -p "Push the topic? [Yn]: " _commit
##    if [[ "X${_commit}" = "X" ]]; then
##      _commit="yes"
##    fi
##  
##    case "${_commit}" in
##      [Nn]*)  _commit=no  ;;
##      *)      _commit=yes  ;;
##    esac
##  
##    if [[ ${_commit} = "yes" ]]; then
##      pushd "${HOME}/.topics" >/dev/null
##      echo git commit -m 'Updated via topic_edit" -- ${file}
##      echo git push
##      popd >/dev/null
##   fi
}

# List all topics.

# List all topics.
unset list_all_topics
function list_all_topics() {
  grep --color=never @topic ${topic_dir}/????.* |\
      sed -e 's/.txt:@topic/:/' -e 's/.md:@topic/:/' -e "s,^${topic_dir}/,,"
}

unset list_some_topics
function list_some_topics() {
  if [[ "${1}" = "0" ]] || [[ "${1}" = "00" ]]; then
      grep --color=never @topic ${topic_dir}/0???.* |\
	  sed -e 's/.txt:@topic/:/' -e 's/.md:@topic/:/' -e "s,^${topic_dir}/,,"
  else
      grep --color=never @topic ${topic_dir}/${1}??.* |\
	  sed -e 's/.txt:@topic/:/' -e 's/.md:@topic/:/' -e "s,^${topic_dir}/,,"
  fi
}

unset list_sections
function list_sections() {
  grep --color=never @topic ${topic_dir}/??00.* |\
      grep --color=never '~~~' |\
      sed -e 's/.txt:@topic/:/' -e 's/.md:@topic/:/' -e "s,^${topic_dir}/,,"
}


# Main entry point.

if [[ $# -eq 0 ]]; then
  if [[ -f .topicrc ]]; then
    [[ ${verbose} -gt 0 ]] && echo "0 .topicrc"
    file=$(find_topic_file "$(cat .topicrc)")
    echo "\$file = \"${file}\""
    display_topic "${file}"
  else
    [[ ${verbose} -gt 0 ]] && echo "0 list"
    list_all_topics
  fi

# List mode
elif [ "${1}" = "-l" ] || [ "${1}" = "--list" ]; then
  [[ ${verbose} -gt 0 ]] && echo "list"
  list_all_topics

# Edit mode
elif [ "${1}" = "-e" ] || [ "${1}" = "--edit" ]; then
  [[ ${verbose} -gt 0 ]] && echo "edit"
  edit_topic "${2}"

# Print the topic
elif [ "${1}" = "-p" ] || [ "${1}" = "--print" ]; then
  [[ ${verbose} -gt 0 ]] && echo "print"
  shift
  for topic in "${@}"; do
    display_topic "${topic}"
  done

# Partial search mode
elif [ "${1}" = "-s" ] || [ "${1}" = "--search" ]; then
  [[ ${verbose} -gt 0 ]] && echo "search"
  shift
  keyword_search "${@}"

# Full search mode
elif [ "${1}" = "-a" ] || [ "${1}" = "--all" ]; then
  [[ ${verbose} -gt 0 ]] && echo "full search"
  shift
  full_context_search "${@}"

# Show sections
elif [ "${1}" = "--sections" ]; then
  [[ ${verbose} -gt 0 ]] && echo "sections"
  list_sections

# Search or print mode
else
#  [[ ${verbose} -gt 0 ]] && echo "else"
  case "${1}" in
    [0-9][0-9])
      list_some_topics "${1}"
      return
      ;;
    -*)
      echo "Unsupported option: \"${1}\""
      return
      ;;
  esac

  file="`find_topic_file $1`"
  # if the first term is a file, then assume all are.  Otherwise, search.
  if [[ "X${file}" != "X" ]]; then
    for topic in "${@}"; do
      display_topic "${topic}"
    done
  else
    keyword_search "${@}"
  fi

fi # end search or print.


# Decrypt?
## decrypt=no
## if [ "${1}" = "-d" -o "${1}" = "-g" -o "${1}" = "--decrypt" -o "${1}" = "--gpg" ]; then
##   decrypt=yes
##   shift
## fi
