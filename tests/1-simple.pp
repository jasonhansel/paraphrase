#define(h)(Hello);
#define(w)(world);
#h #w!
#define(hw)(#h #w);
#hw
#if_eq(#h)(Hello){X}{#h}
#if_eq(#h)(Mello){#h}{Y}
