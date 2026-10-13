/* Test-only identity-preserving Landlock launcher. No production bypass.
 * Requires ABI 9 for pathname Unix socket denial; inherited service seccomp
 * restricts address families separately. Never continue after a policy error.
 */
#define _GNU_SOURCE
#include <fcntl.h>
#include <linux/landlock.h>
#include <stdint.h>
#include <stddef.h>
#include <sys/stat.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <unistd.h>
static void fail(const char *message) {perror(message);exit(125);}
static void allow(int rules,const char *path,uint64_t rights) {
 int fd=open(path,O_PATH|O_CLOEXEC);if(fd<0)fail("sandbox allow path");
 struct landlock_path_beneath_attr rule={.allowed_access=rights,.parent_fd=fd};
 if(syscall(SYS_landlock_add_rule,rules,LANDLOCK_RULE_PATH_BENEATH,&rule,0))fail("sandbox path policy");
 close(fd);
}
int main(int argc,char **argv) {
 if(argc<3 || argv[1][0]!='/' || argv[2][0]!='/') {fputs("Usage: sandbox <private-root> <absolute-test> [args...]\n",stderr);return 125;}
 struct stat root;if(lstat(argv[1],&root) || !S_ISDIR(root.st_mode) || root.st_uid!=getuid() || (root.st_mode&0777)!=0700) {fputs("Private owned root required\n",stderr);return 125;}
 size_t length=strlen(argv[1]);if(strncmp(argv[1],argv[2],length) || argv[2][length]!='/') {fputs("Test must be inside private root\n",stderr);return 125;}
 int abi=syscall(SYS_landlock_create_ruleset,NULL,0,LANDLOCK_CREATE_RULESET_VERSION);if(abi<9) {fputs("Landlock ABI 9 required for host Unix socket denial\n",stderr);return 125;}
 const uint64_t handled=(1ULL<<17)-1;
 struct landlock_ruleset_attr attr={.handled_access_fs=handled,.handled_access_net=0,.scoped=LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET|LANDLOCK_SCOPE_SIGNAL};
 int rules=syscall(SYS_landlock_create_ruleset,&attr,offsetof(struct landlock_ruleset_attr,quiet_access_fs),0);if(rules<0)fail("sandbox ruleset");
 const uint64_t read=LANDLOCK_ACCESS_FS_READ_FILE|LANDLOCK_ACCESS_FS_READ_DIR|LANDLOCK_ACCESS_FS_EXECUTE;
 allow(rules,"/usr",read);
 allow(rules,argv[1],handled&~(LANDLOCK_ACCESS_FS_MAKE_CHAR|LANDLOCK_ACCESS_FS_MAKE_BLOCK|LANDLOCK_ACCESS_FS_IOCTL_DEV));
 for(size_t i=0;i<4;i++) {const char *paths[]={"/dev/null","/dev/zero","/dev/random","/dev/urandom"};allow(rules,paths[i],LANDLOCK_ACCESS_FS_READ_FILE|LANDLOCK_ACCESS_FS_WRITE_FILE);}
 for(size_t i=0;i<4;i++) {const char *paths[]={"/etc/passwd","/etc/group","/etc/nsswitch.conf","/etc/machine-id"};allow(rules,paths[i],LANDLOCK_ACCESS_FS_READ_FILE);}
 /* dbus-daemon EXTERNAL auth consults non-secret seat/user membership files.
  * Read these narrow metadata scopes only; no systemd/user bus socket rights. */
 allow(rules,"/run/systemd/seats",LANDLOCK_ACCESS_FS_READ_DIR|LANDLOCK_ACCESS_FS_READ_FILE);
 char user[80];snprintf(user,sizeof(user),"/run/systemd/users/%lu",(unsigned long)getuid());
 allow(rules,user,LANDLOCK_ACCESS_FS_READ_FILE);
 if(prctl(PR_SET_NO_NEW_PRIVS,1,0,0,0))fail("sandbox no-new-privileges");
 if(syscall(SYS_landlock_restrict_self,rules,0))fail("sandbox enforcement");
 close(rules);
 execv(argv[2],argv+2);fail("sandbox test exec");
}
