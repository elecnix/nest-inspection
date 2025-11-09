#
#    Copyright (c) 2010-2012 Nest Labs, Inc.
#    All rights reserved.
#
#    Description:
#      This file is the default root user set-up file for all Bourne- and
#      Korn-compatible shells.
#

PATH="${PATH}:/nestlabs/sbin:/nestlabs/bin:/nestlabs/diags/bin"
export PATH

ROOTDIR="${PREFIX}/"
NESTLABSDIR="${ROOTDIR}nestlabs"
NESTLABSLIBDIR="${NESTLABSDIR}/lib"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}${LD_LIBRARY_PATH:+:}${NESTLABSLIBDIR}"

if [ -f /media/user-config/.bashrc ]; then
   source /media/user-config/.bashrc
fi
